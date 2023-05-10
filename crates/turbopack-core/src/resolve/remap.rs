use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    ops::Deref,
};

use anyhow::{bail, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    alias_map::{AliasMap, AliasMapIter, AliasPattern, AliasTemplate},
    options::ConditionValue,
};

enum ExportImport {
    Export,
    Import,
}

impl ExportImport {
    fn is_imports(&self) -> bool {
        matches!(self, Self::Import)
    }
}

impl Display for ExportImport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Export => f.write_str("export"),
            Self::Import => f.write_str("import"),
        }
    }
}

/// The result an "exports"/"imports" field describes. Can represent multiple
/// alternatives, conditional result, ignored result (null mapping) and a plain
/// result.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SubpathValue {
    /// Alternative subpaths, defined with `"path": ["other1", "other2"]`,
    /// allows for specifying multiple possible remappings to be tried. This
    /// may be that conditions didn't match, or that a particular path
    /// wasn't found.
    Alternatives(Vec<SubpathValue>),

    /// Conditional subpaths, defined with `"path": { "condition": "other"}`,
    /// allow remapping based on certain predefined conditions. Eg, if using
    /// ESM import syntax, the `import` condition allows you to remap to a
    /// file that uses ESM syntax.
    /// Node defines several conditions in https://nodejs.org/api/packages.html#conditional-exports
    /// TODO: Should this use an enum of predefined keys?
    Conditional(Vec<(String, SubpathValue)>),

    /// A result subpath, defined with `"path": "other"`, remaps imports of
    /// `path` to `other`.
    Result(String),

    /// An excluded subpath, defined with `"path": null`, prevents importing
    /// this subpath.
    Excluded,
}

impl AliasTemplate for SubpathValue {
    type Output<'a> = Result<Self> where Self: 'a;

    fn replace(&self, capture: &str) -> Result<Self> {
        Ok(match self {
            SubpathValue::Alternatives(list) => SubpathValue::Alternatives(
                list.iter()
                    .map(|value| value.replace(capture))
                    .collect::<Result<Vec<_>>>()?,
            ),
            SubpathValue::Conditional(list) => SubpathValue::Conditional(
                list.iter()
                    .map(|(condition, value)| Ok((condition.clone(), value.replace(capture)?)))
                    .collect::<Result<Vec<_>>>()?,
            ),
            SubpathValue::Result(value) => SubpathValue::Result(value.replace('*', capture)),
            SubpathValue::Excluded => SubpathValue::Excluded,
        })
    }
}

impl SubpathValue {
    /// Returns an iterator over all leaf results.
    fn results_mut(&mut self) -> ResultsIterMut<'_> {
        ResultsIterMut { stack: vec![self] }
    }

    /// Walks the [SubpathValue] and adds results to the `target` vector. It
    /// uses the `conditions` to skip or enter conditional results.
    /// The state of conditions is stored within `condition_overrides`, which is
    /// also exposed to the consumer.
    pub fn add_results<'a>(
        &'a self,
        conditions: &BTreeMap<String, ConditionValue>,
        unspecified_condition: &ConditionValue,
        condition_overrides: &mut HashMap<&'a str, ConditionValue>,
        target: &mut Vec<&'a str>,
    ) -> bool {
        match self {
            SubpathValue::Alternatives(list) => {
                for value in list {
                    if value.add_results(
                        conditions,
                        unspecified_condition,
                        condition_overrides,
                        target,
                    ) {
                        return true;
                    }
                }
                false
            }
            SubpathValue::Conditional(list) => {
                for (condition, value) in list {
                    let condition_value = if condition == "default" {
                        &ConditionValue::Set
                    } else {
                        condition_overrides
                            .get(condition.as_str())
                            .or_else(|| conditions.get(condition))
                            .unwrap_or(unspecified_condition)
                    };
                    match condition_value {
                        ConditionValue::Set => {
                            if value.add_results(
                                conditions,
                                unspecified_condition,
                                condition_overrides,
                                target,
                            ) {
                                return true;
                            }
                        }
                        ConditionValue::Unset => {}
                        ConditionValue::Unknown => {
                            condition_overrides.insert(condition, ConditionValue::Set);
                            if value.add_results(
                                conditions,
                                unspecified_condition,
                                condition_overrides,
                                target,
                            ) {
                                condition_overrides.insert(condition, ConditionValue::Unset);
                            } else {
                                condition_overrides.remove(condition.as_str());
                            }
                        }
                    }
                }
                false
            }
            SubpathValue::Result(r) => {
                target.push(r);
                true
            }
            SubpathValue::Excluded => true,
        }
    }

    fn try_from(value: &Value, ty: ExportImport) -> Result<Self> {
        match value {
            Value::Null => Ok(SubpathValue::Excluded),
            Value::String(s) => Ok(SubpathValue::Result(s.to_string())),
            Value::Number(_) => bail!("numeric values are invalid in {ty}s field entries"),
            Value::Bool(_) => bail!("boolean values are invalid in {ty}s field entries"),
            Value::Object(object) => Ok(SubpathValue::Conditional(
                object
                    .iter()
                    .map(|(key, value)| {
                        if key.starts_with('.') {
                            bail!(
                                "invalid key \"{}\" in an {ty} field conditions object. Did you \
                                 mean to place this request at a higher level?",
                                key
                            );
                        }

                        Ok((key.to_string(), SubpathValue::try_from(value, ty)?))
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Value::Array(array) => Ok(SubpathValue::Alternatives(
                array
                    .iter()
                    .map(|value| SubpathValue::try_from(value, ty))
                    .collect::<Result<Vec<_>>>()?,
            )),
        }
    }
}

struct ResultsIterMut<'a> {
    stack: Vec<&'a mut SubpathValue>,
}

impl<'a> Iterator for ResultsIterMut<'a> {
    type Item = &'a mut String;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(value) = self.stack.pop() {
            match value {
                SubpathValue::Alternatives(list) => {
                    for value in list {
                        self.stack.push(value);
                    }
                }
                SubpathValue::Conditional(list) => {
                    for (_, value) in list {
                        self.stack.push(value);
                    }
                }
                SubpathValue::Result(r) => return Some(r),
                SubpathValue::Excluded => {}
            }
        }
        None
    }
}

/// Content of an "exports" field in a package.json
#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportsField(AliasMap<SubpathValue>);

impl TryFrom<&Value> for ExportsField {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self> {
        // The "exports" field can be an object, a string, or an array of strings.
        // https://nodejs.org/api/packages.html#exports
        let map = match value {
            Value::Object(object) => {
                let mut map = AliasMap::new();
                // Conditional exports can also be defined at the top-level of the
                // exports field, where they will apply to the package itself.
                let mut conditions = vec![];

                for (key, value) in object.iter() {
                    // NOTE: Node.js does not allow conditional and non-conditional keys
                    // to be mixed at the top-level, but we do.
                    if key != "." && !key.starts_with("./") {
                        conditions.push((key, value));
                        continue;
                    }

                    let mut value = SubpathValue::try_from(value, ExportImport::Export)?;

                    let pattern = if is_folder_shorthand(key) {
                        expand_folder_shorthand(key, &mut value)?
                    } else {
                        AliasPattern::parse(key)
                    };

                    map.insert(pattern, value);
                }

                if !conditions.is_empty() {
                    map.insert(
                        AliasPattern::Exact(".".to_string()),
                        SubpathValue::Conditional(
                            conditions
                                .into_iter()
                                .map(|(key, value)| {
                                    Ok((
                                        key.to_string(),
                                        SubpathValue::try_from(value, ExportImport::Export)?,
                                    ))
                                })
                                .collect::<Result<Vec<_>>>()?,
                        ),
                    );
                }

                map
            }
            Value::String(string) => {
                let mut map = AliasMap::new();
                map.insert(
                    AliasPattern::exact("."),
                    SubpathValue::Result(string.to_string()),
                );
                map
            }
            Value::Array(array) => {
                let mut map = AliasMap::new();
                map.insert(
                    AliasPattern::exact("."),
                    // This allows for more complex patterns than the spec allows, since we accept
                    // the following:
                    // [{ "node": "./node.js", "default": "./index.js" }, "./index.js"]
                    SubpathValue::Alternatives(
                        array
                            .iter()
                            .map(|value| SubpathValue::try_from(value, ExportImport::Export))
                            .collect::<Result<Vec<_>>>()?,
                    ),
                );
                map
            }
            _ => {
                bail!("\"exports\" field must be an object or a string");
            }
        };
        Ok(Self(map))
    }
}

impl Deref for ExportsField {
    type Target = AliasMap<SubpathValue>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Content of an "imports" field in a package.json
#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportsField(AliasMap<SubpathValue>);

impl TryFrom<&Value> for ImportsField {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self> {
        // The "imports" field must be an object.
        // https://nodejs.org/api/packages.html#imports
        let map = match value {
            Value::Object(object) => {
                let mut map = AliasMap::new();

                for (key, value) in object.iter() {
                    if !key.starts_with("#") {
                        bail!("imports key \"{key}\" must begin with a '#'")
                    }
                    let value = SubpathValue::try_from(value, ExportImport::Import)?;
                    map.insert(AliasPattern::parse(key), value);
                }

                map
            }
            _ => bail!("\"imports\" field must be an object"),
        };
        Ok(Self(map))
    }
}

impl Deref for ImportsField {
    type Target = AliasMap<SubpathValue>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Returns true if the given string is a folder path shorthand.
fn is_folder_shorthand(key: &str) -> bool {
    key.ends_with('/') && key.find('*').is_none()
}

/// The exports field supports a shorthand for folders, where:
///   "./folder/": "./other-folder/"
/// is equivalent to
///   "./folder/*": "./other-folder/*"
/// This is not implemented directly by [`AliasMap`] as it is not
/// shared behavior with the tsconfig.json `paths` field. Instead,
/// we do the expansion here.
fn expand_folder_shorthand(key: &str, value: &mut SubpathValue) -> Result<AliasPattern> {
    // Transform folder patterns into wildcard patterns.
    let pattern = AliasPattern::wildcard(key, "");

    // Transform templates into wildcard patterns as well.
    for result in value.results_mut() {
        if result.ends_with('/') {
            if result.find('*').is_none() {
                result.push('*');
            } else {
                bail!(
                    "invalid exports field value \"{}\" for key \"{}\": \"*\" is not allowed in \
                     folder exports",
                    result,
                    key
                );
            }
        } else {
            bail!(
                "invalid exports field value \"{}\" for key \"{}\": folder exports must end with \
                 \"/\"",
                result,
                key
            );
        }
    }

    Ok(pattern)
}

/// Content of an "alias" configuration
#[turbo_tasks::value(shared)]
#[derive(Default)]
pub struct ResolveAliasMap(#[turbo_tasks(trace_ignore)] AliasMap<SubpathValue>);

impl TryFrom<&IndexMap<String, Value>> for ResolveAliasMap {
    type Error = anyhow::Error;

    fn try_from(object: &IndexMap<String, Value>) -> Result<Self> {
        let mut map = AliasMap::new();

        for (key, value) in object.iter() {
            let mut value = SubpathValue::try_from(value, ExportImport::Export)?;

            let pattern = if is_folder_shorthand(key) {
                expand_folder_shorthand(key, &mut value)?
            } else {
                AliasPattern::parse(key)
            };

            map.insert(pattern, value);
        }
        Ok(Self(map))
    }
}

impl<'a> IntoIterator for &'a ResolveAliasMap {
    type Item = (AliasPattern, &'a SubpathValue);
    type IntoIter = AliasMapIter<'a, SubpathValue>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}
