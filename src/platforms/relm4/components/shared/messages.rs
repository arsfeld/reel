use relm4::prelude::*;

#[derive(Debug, Clone)]
pub enum AppInput {
    Initialize,
    Navigate(NavigationTarget),
    ShowError(String),
    ShowLoading(bool),
    Quit,
}

#[derive(Debug, Clone)]
pub enum NavigationTarget {
    Home,
    Library {
        source_id: String,
        library_id: String,
    },
    MovieDetails {
        media_id: String,
    },
    ShowDetails {
        media_id: String,
    },
    Player {
        media_id: String,
    },
    Sources,
    Preferences,
}

#[derive(Debug, Clone)]
pub enum AppOutput {
    NavigationChanged(NavigationTarget),
}

#[derive(Debug, Clone)]
pub enum CommonInput<T> {
    Data(T),
    Loading,
    Error(String),
    Refresh,
}

#[derive(Debug, Clone)]
pub enum CommonOutput {
    NavigationRequested(NavigationTarget),
    DataRequested(String),
    ActionRequested(String),
}
