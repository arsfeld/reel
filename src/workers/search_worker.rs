use crate::models::{MediaItem, MediaItemId};
use crate::ui::shared::broker::{BROKER, BrokerMessage, DataMessage};
use relm4::{ComponentSender, Worker};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tantivy::{
    Index, IndexReader, IndexWriter,
    collector::TopDocs,
    directory::MmapDirectory,
    doc,
    query::QueryParser,
    schema::{Field, STORED, Schema, TEXT, Value},
};
use tracing::{error, info, trace};

#[derive(Debug, Clone)]
pub struct SearchDocument {
    pub id: MediaItemId,
    pub title: String,
    pub overview: Option<String>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
}

impl From<MediaItem> for SearchDocument {
    fn from(item: MediaItem) -> Self {
        match item {
            MediaItem::Movie(movie) => Self {
                id: MediaItemId::from(movie.id.clone()),
                title: movie.title,
                overview: movie.overview,
                year: movie.year.map(|y| y as i32),
                genres: movie.genres,
            },
            MediaItem::Show(show) => Self {
                id: MediaItemId::from(show.id.clone()),
                title: show.title,
                overview: show.overview,
                year: show.year.map(|y| y as i32),
                genres: show.genres,
            },
            MediaItem::Episode(episode) => Self {
                id: MediaItemId::from(episode.id.clone()),
                title: episode.title,
                overview: episode.overview,
                year: None,
                genres: Vec::new(),
            },
            _ => Self {
                id: MediaItemId::from(item.id().to_string()),
                title: item.title().to_string(),
                overview: None,
                year: None,
                genres: Vec::new(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum SearchWorkerInput {
    BrokerMsg(BrokerMessage),
    LoadInitialIndex { db: Arc<DatabaseConnection> },
    IndexDocuments(Vec<SearchDocument>),
    UpdateDocument(SearchDocument),
    RemoveDocument(MediaItemId),
    Search { query: String, limit: usize },
    ClearIndex,
    OptimizeIndex,
}

#[derive(Debug, Clone)]
pub enum SearchWorkerOutput {
    SearchResults {
        query: String,
        results: Vec<MediaItemId>,
        total_hits: usize,
    },
    IndexingComplete {
        documents_indexed: usize,
    },
    DocumentUpdated {
        id: MediaItemId,
    },
    DocumentRemoved {
        id: MediaItemId,
    },
    IndexCleared,
    IndexOptimized,
    Error(String),
}

pub struct SearchWorker {
    index: Option<Index>,
    reader: Option<IndexReader>,
    writer: Option<IndexWriter>,
    id_field: Field,
    title_field: Field,
    overview_field: Field,
    year_field: Field,
    genres_field: Field,
}

impl SearchWorker {
    fn new() -> Result<Self, String> {
        let index_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("reel")
            .join("search_index");

        std::fs::create_dir_all(&index_dir)
            .map_err(|e| format!("Failed to create index directory: {}", e))?;

        // Build schema
        let mut schema_builder = Schema::builder();
        let id_field = schema_builder.add_text_field("id", STORED);
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let overview_field = schema_builder.add_text_field("overview", TEXT);
        let year_field = schema_builder.add_text_field("year", TEXT | STORED);
        let genres_field = schema_builder.add_text_field("genres", TEXT);
        let schema = schema_builder.build();

        // Create or open index
        let index = if index_dir.join("meta.json").exists() {
            let mmap_dir = MmapDirectory::open(&index_dir)
                .map_err(|e| format!("Failed to open index directory: {}", e))?;
            Index::open(mmap_dir).map_err(|e| format!("Failed to open index: {}", e))?
        } else {
            let _mmap_dir = MmapDirectory::open(&index_dir)
                .map_err(|e| format!("Failed to open index directory: {}", e))?;
            Index::create_in_dir(&index_dir, schema.clone())
                .map_err(|e| format!("Failed to create index: {}", e))?
        };

        let reader = index
            .reader()
            .map_err(|e| format!("Failed to create index reader: {}", e))?;

        let writer = index
            .writer(50_000_000) // 50MB writer buffer
            .map_err(|e| format!("Failed to create index writer: {}", e))?;

        Ok(Self {
            index: Some(index),
            reader: Some(reader),
            writer: Some(writer),
            id_field,
            title_field,
            overview_field,
            year_field,
            genres_field,
        })
    }

    fn index_documents(&mut self, documents: Vec<SearchDocument>) -> Result<usize, String> {
        // Check if we have a writer available
        if self.writer.is_none() {
            return Err("Search index not available".to_string());
        }

        // Deduplicate documents by ID, keeping the last occurrence (most recent data)
        let mut unique_docs: HashMap<MediaItemId, SearchDocument> = HashMap::new();
        for doc in documents {
            unique_docs.insert(doc.id.clone(), doc);
        }

        let count = unique_docs.len();

        for doc in unique_docs.into_values() {
            self.add_document(doc)?;
        }

        // Now we can safely access the writer
        if let Some(writer) = self.writer.as_mut() {
            writer
                .commit()
                .map_err(|e| format!("Failed to commit index: {}", e))?;
        }

        info!("Indexed {} documents", count);
        Ok(count)
    }

    fn add_document(&mut self, doc: SearchDocument) -> Result<(), String> {
        // Remove existing document if it exists
        self.remove_document_internal(&doc.id)?;

        let mut tantivy_doc = doc!(
            self.id_field => doc.id.to_string(),
            self.title_field => doc.title.clone()
        );

        if let Some(overview) = &doc.overview {
            tantivy_doc.add_text(self.overview_field, overview);
        }

        if let Some(year) = doc.year {
            tantivy_doc.add_text(self.year_field, year.to_string());
        }

        if !doc.genres.is_empty() {
            tantivy_doc.add_text(self.genres_field, doc.genres.join(" "));
        }

        // Check if we have a writer available and add the document
        if let Some(writer) = self.writer.as_mut() {
            writer
                .add_document(tantivy_doc)
                .map_err(|e| format!("Failed to add document: {}", e))?;
        } else {
            return Err("Search index not available".to_string());
        }

        Ok(())
    }

    fn remove_document_internal(&mut self, id: &MediaItemId) -> Result<(), String> {
        // Check if we have a writer available
        if let Some(writer) = self.writer.as_mut() {
            let term = tantivy::Term::from_field_text(self.id_field, id.as_ref());
            writer.delete_term(term);
        }
        Ok(())
    }

    fn search(&self, query_str: &str, limit: usize) -> Result<(Vec<MediaItemId>, usize), String> {
        // Check if we have index and reader available
        let index = self
            .index
            .as_ref()
            .ok_or_else(|| "Search index not available".to_string())?;
        let reader = self
            .reader
            .as_ref()
            .ok_or_else(|| "Search index not available".to_string())?;
        let searcher = reader.searcher();

        // Create query parser for multiple fields
        let query_parser = QueryParser::for_index(
            index,
            vec![self.title_field, self.overview_field, self.genres_field],
        );

        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| format!("Failed to parse query: {}", e))?;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| format!("Failed to search: {}", e))?;

        let mut results = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for (_score, doc_address) in &top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher
                .doc(*doc_address)
                .map_err(|e| format!("Failed to retrieve document: {}", e))?;

            if let Some(id_value) = retrieved_doc.get_first(self.id_field) {
                if let Some(id_str) = id_value.as_str() {
                    let id = id_str.parse::<MediaItemId>().unwrap();

                    // Deduplicate: only add if we haven't seen this ID before
                    // This preserves the highest-scoring result since results are sorted by score
                    if seen_ids.insert(id.clone()) {
                        results.push(id);
                    }
                }
            }
        }

        // Return deduplicated results count, not original top_docs.len()
        let total_unique = results.len();
        Ok((results, total_unique))
    }

    fn clear_index(&mut self) -> Result<(), String> {
        // Check if we have a writer available
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| "Search index not available".to_string())?;

        writer
            .delete_all_documents()
            .map_err(|e| format!("Failed to clear index: {}", e))?;

        writer
            .commit()
            .map_err(|e| format!("Failed to commit after clearing: {}", e))?;

        info!("Index cleared");
        Ok(())
    }

    fn optimize_index(&mut self) -> Result<(), String> {
        // Check if we have a writer available
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| "Search index not available".to_string())?;

        writer
            .commit()
            .map_err(|e| format!("Failed to commit before optimization: {}", e))?;

        // Note: Tantivy handles optimization differently in newer versions
        // The writer automatically merges segments as needed
        info!("Index optimized");
        Ok(())
    }
}

impl std::fmt::Debug for SearchWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchWorker")
            .field("index", &self.index.is_some())
            .field("reader", &self.reader.is_some())
            .field("writer", &self.writer.is_some())
            .finish()
    }
}

impl Worker for SearchWorker {
    type Init = Arc<DatabaseConnection>;
    type Input = SearchWorkerInput;
    type Output = SearchWorkerOutput;

    fn init(db: Self::Init, sender: ComponentSender<Self>) -> Self {
        // Subscribe to broker for media updates
        // Create a wrapper sender that maps BrokerMessage to SearchWorkerInput
        let input_sender = sender.input_sender().clone();
        relm4::spawn(async move {
            // Create a channel to receive broker messages
            let (broker_sender, broker_receiver) = relm4::channel::<BrokerMessage>();
            BROKER
                .subscribe("SearchWorker".to_string(), broker_sender)
                .await;

            // Forward broker messages as SearchWorkerInput
            while let Some(msg) = broker_receiver.recv().await {
                input_sender.send(SearchWorkerInput::BrokerMsg(msg)).ok();
            }
        });

        // Load initial index from database
        sender.input(SearchWorkerInput::LoadInitialIndex { db: db.clone() });

        match Self::new() {
            Ok(worker) => worker,
            Err(e) => {
                error!(
                    "Failed to initialize search worker: {}. Creating fallback worker.",
                    e
                );
                // Send error message to inform the component
                sender
                    .output(SearchWorkerOutput::Error(format!(
                        "Search index unavailable: {}",
                        e
                    )))
                    .ok();

                // Return a worker with None values - it will handle searches by returning empty results
                // We still need to create the field definitions even without an index
                let mut schema_builder = Schema::builder();
                let id_field = schema_builder.add_text_field("id", STORED);
                let title_field = schema_builder.add_text_field("title", TEXT | STORED);
                let overview_field = schema_builder.add_text_field("overview", TEXT);
                let year_field = schema_builder.add_text_field("year", TEXT | STORED);
                let genres_field = schema_builder.add_text_field("genres", TEXT);

                SearchWorker {
                    index: None,
                    writer: None,
                    reader: None,
                    id_field,
                    title_field,
                    overview_field,
                    year_field,
                    genres_field,
                }
            }
        }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SearchWorkerInput::BrokerMsg(broker_msg) => {
                match broker_msg {
                    BrokerMessage::Data(DataMessage::MediaUpdated { media_id }) => {
                        trace!("Received MediaUpdated event for: {}", media_id);
                        // Single item update - would need to fetch from DB to index
                        // For now, we'll rely on batch updates during sync
                    }
                    BrokerMessage::Data(DataMessage::MediaBatchSaved { items }) => {
                        info!("Received MediaBatchSaved event with {} items", items.len());
                        // Convert MediaItemModel to MediaItem to SearchDocument
                        let documents: Vec<SearchDocument> = items
                            .into_iter()
                            .filter_map(|model| {
                                // Try to convert MediaItemModel to MediaItem
                                match MediaItem::try_from(model) {
                                    Ok(media_item) => Some(SearchDocument::from(media_item)),
                                    Err(e) => {
                                        error!("Failed to convert media item for indexing: {}", e);
                                        None
                                    }
                                }
                            })
                            .collect();

                        if !documents.is_empty() {
                            match self.index_documents(documents) {
                                Ok(count) => {
                                    info!("Indexed {} documents from batch save", count);
                                }
                                Err(e) => {
                                    error!("Failed to index batch: {}", e);
                                }
                            }
                        }
                    }
                    _ => {} // Ignore other broker messages
                }
            }
            SearchWorkerInput::LoadInitialIndex { db } => {
                info!("Loading initial search index from database");
                use crate::db::repository::{Repository, media_repository::MediaRepositoryImpl};

                let sender_clone = sender.clone();
                relm4::spawn(async move {
                    let repo = MediaRepositoryImpl::new(db);
                    match repo.find_all().await {
                        Ok(models) => {
                            info!("Found {} media items for initial indexing", models.len());

                            // Convert to SearchDocuments
                            let documents: Vec<SearchDocument> = models
                                .into_iter()
                                .filter_map(|model| match MediaItem::try_from(model) {
                                    Ok(media_item) => Some(SearchDocument::from(media_item)),
                                    Err(e) => {
                                        error!("Failed to convert media item: {}", e);
                                        None
                                    }
                                })
                                .collect();

                            if !documents.is_empty() {
                                sender_clone.input(SearchWorkerInput::IndexDocuments(documents));
                            }
                        }
                        Err(e) => {
                            error!("Failed to load media items for initial index: {}", e);
                        }
                    }
                });
            }
            SearchWorkerInput::IndexDocuments(documents) => match self.index_documents(documents) {
                Ok(count) => {
                    sender
                        .output(SearchWorkerOutput::IndexingComplete {
                            documents_indexed: count,
                        })
                        .ok();
                }
                Err(e) => {
                    sender.output(SearchWorkerOutput::Error(e)).ok();
                }
            },

            SearchWorkerInput::UpdateDocument(document) => {
                let id = document.id.clone();
                match self.add_document(document) {
                    Ok(_) => {
                        if let Some(writer) = self.writer.as_mut() {
                            if let Err(e) = writer.commit() {
                                sender
                                    .output(SearchWorkerOutput::Error(format!(
                                        "Failed to commit update: {}",
                                        e
                                    )))
                                    .ok();
                            } else {
                                sender
                                    .output(SearchWorkerOutput::DocumentUpdated { id })
                                    .ok();
                            }
                        } else {
                            sender
                                .output(SearchWorkerOutput::Error(
                                    "Search index not available".to_string(),
                                ))
                                .ok();
                        }
                    }
                    Err(e) => {
                        sender.output(SearchWorkerOutput::Error(e)).ok();
                    }
                }
            }

            SearchWorkerInput::RemoveDocument(id) => match self.remove_document_internal(&id) {
                Ok(_) => {
                    if let Some(writer) = self.writer.as_mut() {
                        if let Err(e) = writer.commit() {
                            sender
                                .output(SearchWorkerOutput::Error(format!(
                                    "Failed to commit removal: {}",
                                    e
                                )))
                                .ok();
                        } else {
                            sender
                                .output(SearchWorkerOutput::DocumentRemoved { id })
                                .ok();
                        }
                    } else {
                        sender
                            .output(SearchWorkerOutput::Error(
                                "Search index not available".to_string(),
                            ))
                            .ok();
                    }
                }
                Err(e) => {
                    sender.output(SearchWorkerOutput::Error(e)).ok();
                }
            },

            SearchWorkerInput::Search { query, limit } => match self.search(&query, limit) {
                Ok((results, total_hits)) => {
                    sender
                        .output(SearchWorkerOutput::SearchResults {
                            query,
                            results,
                            total_hits,
                        })
                        .ok();
                }
                Err(e) => {
                    sender.output(SearchWorkerOutput::Error(e)).ok();
                }
            },

            SearchWorkerInput::ClearIndex => match self.clear_index() {
                Ok(_) => {
                    sender.output(SearchWorkerOutput::IndexCleared).ok();
                }
                Err(e) => {
                    sender.output(SearchWorkerOutput::Error(e)).ok();
                }
            },

            SearchWorkerInput::OptimizeIndex => match self.optimize_index() {
                Ok(_) => {
                    sender.output(SearchWorkerOutput::IndexOptimized).ok();
                }
                Err(e) => {
                    sender.output(SearchWorkerOutput::Error(e)).ok();
                }
            },
        }
    }
}
