use crate::{
    app::constant::{COMMA, COMMA_STRING},
    chat::{config::key_config, constant::Models},
};
use serde::{Deserialize, Serialize};
// use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(Clone, PartialEq)]
pub enum UsageCheck {
    None,
    Default,
    All,
    Custom(Vec<String>),
}

impl UsageCheck {
    pub fn from_proto(model: Option<&key_config::UsageCheckModel>) -> Option<Self> {
        model.map(|model| {
            use key_config::usage_check_model::Type;
            match Type::try_from(model.r#type).unwrap_or(Type::Default) {
                Type::Default | Type::Disabled => Self::None,
                Type::All => Self::All,
                Type::Custom => {
                    let models: Vec<_> = model
                        .model_ids
                        .iter()
                        .filter_map(|id| Models::find_id(id))
                        .collect();
                    if models.is_empty() {
                        Self::None
                    } else {
                        Self::Custom(models)
                    }
                }
            }
        })
    }

    // pub fn to_proto(&self) -> key_config::UsageCheckModel {
    //     use key_config::usage_check_model::Type;
    //     match self {
    //         Self::None => key_config::UsageCheckModel {
    //             r#type: Type::Disabled.into(),
    //             model_ids: vec![],
    //         },
    //         Self::Default => key_config::UsageCheckModel {
    //             r#type: Type::Default.into(),
    //             model_ids: vec![],
    //         },
    //         Self::All => key_config::UsageCheckModel {
    //             r#type: Type::All.into(),
    //             model_ids: vec![],
    //         },
    //         Self::Custom(models) => key_config::UsageCheckModel {
    //             r#type: Type::Custom.into(),
    //             model_ids: models.iter().map(|&s| s.to_string()).collect(),
    //         },
    //     }
    // }
}

impl Default for UsageCheck {
    fn default() -> Self {
        Self::Default
    }
}

impl Serialize for UsageCheck {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("UsageCheck", 1)?;
        match self {
            UsageCheck::None => {
                state.serialize_field("type", "none")?;
            }
            UsageCheck::Default => {
                state.serialize_field("type", "default")?;
            }
            UsageCheck::All => {
                state.serialize_field("type", "all")?;
            }
            UsageCheck::Custom(models) => {
                state.serialize_field("type", "list")?;
                state.serialize_field("content", &models.join(COMMA_STRING))?;
            }
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for UsageCheck {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "type", content = "content")]
        enum UsageCheckHelper {
            #[serde(rename = "none")]
            None,
            #[serde(rename = "default")]
            Default,
            #[serde(rename = "all")]
            All,
            #[serde(rename = "list")]
            Custom(String),
        }

        let helper = <UsageCheckHelper as serde::Deserialize>::deserialize(deserializer)?;
        Ok(match helper {
            UsageCheckHelper::None => UsageCheck::None,
            UsageCheckHelper::Default => UsageCheck::Default,
            UsageCheckHelper::All => UsageCheck::All,
            UsageCheckHelper::Custom(list) => {
                if list.is_empty() {
                    return Ok(UsageCheck::None);
                }

                let models: Vec<_> = list
                    .split(COMMA)
                    .filter_map(|model| {
                        let model = model.trim();
                        Models::find_id(model)
                    })
                    .collect();

                if models.is_empty() {
                    UsageCheck::None
                } else {
                    UsageCheck::Custom(models)
                }
            }
        })
    }
}

impl UsageCheck {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "none" | "disabled" => Self::None,
            "default" => Self::Default,
            "all" | "everything" => Self::All,
            list => {
                if list.is_empty() {
                    return Self::default();
                }
                let models: Vec<_> = list
                    .split(COMMA)
                    .filter_map(|model| {
                        let model = model.trim();
                        Models::find_id(model)
                    })
                    .collect();

                if models.is_empty() {
                    Self::default()
                } else {
                    Self::Custom(models)
                }
            }
        }
    }
}
