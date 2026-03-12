use std::{fmt, time::SystemTime};

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub(crate) struct $name(pub String);

        impl $name {
            pub(crate) fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

string_id!(CompanyId);
string_id!(AgentId);
string_id!(WorkId);
string_id!(LeaseId);
string_id!(SessionId);
string_id!(RecordId);
string_id!(RunId);
string_id!(ContractSetId);
string_id!(ActorId);

pub(crate) type Rev = u64;
pub(crate) type ContractRev = u32;
pub(crate) type Timestamp = SystemTime;
