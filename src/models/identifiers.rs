use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

macro_rules! impl_id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = std::convert::Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.to_string()))
            }
        }
    };
}

impl_id_type!(SourceId);
impl_id_type!(BackendId);
impl_id_type!(ProviderId);
impl_id_type!(LibraryId);
impl_id_type!(MediaItemId);
impl_id_type!(ShowId);
impl_id_type!(UserId);

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_id_type {
        ($name:ident) => {
            mod $name {
                use super::*;

                #[test]
                fn test_creation_and_conversion() {
                    let id = $name::new("test_id");
                    assert_eq!(id.as_str(), "test_id");
                    assert_eq!(id.to_string(), "test_id");
                }

                #[test]
                fn test_from_string() {
                    let id = $name::from("test_id".to_string());
                    assert_eq!(id.as_str(), "test_id");
                }

                #[test]
                fn test_from_str() {
                    let id = $name::from("test_id");
                    assert_eq!(id.as_str(), "test_id");
                }

                #[test]
                fn test_equality() {
                    let id1 = $name::new("test_id");
                    let id2 = $name::new("test_id");
                    let id3 = $name::new("other_id");

                    assert_eq!(id1, id2);
                    assert_ne!(id1, id3);
                }

                #[test]
                fn test_hashing() {
                    use std::collections::HashSet;

                    let mut set = HashSet::new();
                    let id1 = $name::new("test_id");
                    let id2 = $name::new("test_id");
                    let id3 = $name::new("other_id");

                    set.insert(id1.clone());
                    assert!(set.contains(&id2));
                    assert!(!set.contains(&id3));
                }

                #[test]
                fn test_serialization() {
                    let id = $name::new("test_id");
                    let json = serde_json::to_string(&id).unwrap();
                    assert_eq!(json, "\"test_id\"");

                    let deserialized: $name = serde_json::from_str(&json).unwrap();
                    assert_eq!(deserialized, id);
                }

                #[test]
                fn test_debug() {
                    let id = $name::new("test_id");
                    let debug_str = format!("{:?}", id);
                    assert!(debug_str.contains("test_id"));
                }

                #[test]
                fn test_clone() {
                    let id1 = $name::new("test_id");
                    let id2 = id1.clone();
                    assert_eq!(id1, id2);
                }
            }
        };
    }

    test_id_type!(SourceId);
    test_id_type!(BackendId);
    test_id_type!(ProviderId);
    test_id_type!(LibraryId);
    test_id_type!(MediaItemId);
    test_id_type!(ShowId);
    test_id_type!(UserId);
}
