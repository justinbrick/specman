use serde_yaml::{Mapping, Value};
use specman::front_matter;

use crate::error::{CliError, ExitStatus};

pub fn update_spec_document(
    content: &str,
    name: &str,
    version: &str,
    dependencies: &[String],
) -> Result<String, CliError> {
    rewrite_front_matter(content, |doc| {
        doc.insert(Value::from("name"), Value::from(name));
        doc.insert(Value::from("version"), Value::from(version));
        if !dependencies.is_empty() {
            let mut sequence = current_sequence(doc, "dependencies");
            for dep in dependencies {
                sequence.push(Value::from(dep.clone()));
            }
            doc.insert(Value::from("dependencies"), Value::Sequence(sequence));
        }
        Ok(())
    })
}

pub fn update_impl_document(
    content: &str,
    name: &str,
    spec_locator: &str,
    language: &str,
    location: &str,
) -> Result<String, CliError> {
    rewrite_front_matter(content, |doc| {
        doc.insert(Value::from("name"), Value::from(name));
        doc.insert(Value::from("spec"), Value::from(spec_locator));
        doc.insert(Value::from("location"), Value::from(location));

        let mut language_map = Mapping::new();
        language_map.insert(Value::from("language"), Value::from(language));
        if !language_map.contains_key(Value::from("properties")) {
            language_map.insert(Value::from("properties"), Value::Mapping(Mapping::new()));
        }
        if !language_map.contains_key(Value::from("libraries")) {
            language_map.insert(Value::from("libraries"), Value::Sequence(Vec::new()));
        }
        doc.insert(
            Value::from("primary_language"),
            Value::Mapping(language_map),
        );

        let mut references = Mapping::new();
        references.insert(Value::from("ref"), Value::from(spec_locator));
        references.insert(Value::from("type"), Value::from("specification"));
        references.insert(Value::from("optional"), Value::from(false));
        doc.insert(
            Value::from("references"),
            Value::Sequence(vec![Value::Mapping(references)]),
        );
        Ok(())
    })
}

pub fn update_scratch_document(
    content: &str,
    target: &str,
    branch: &str,
    work_type: &str,
) -> Result<String, CliError> {
    rewrite_front_matter(content, |doc| {
        doc.insert(Value::from("target"), Value::from(target));
        doc.insert(Value::from("branch"), Value::from(branch));
        let mut work_map = Mapping::new();
        work_map.insert(Value::from(work_type), Value::Mapping(Mapping::new()));
        doc.insert(Value::from("work_type"), Value::Mapping(work_map));
        Ok(())
    })
}

fn rewrite_front_matter<F>(content: &str, mut f: F) -> Result<String, CliError>
where
    F: FnMut(&mut Mapping) -> Result<(), CliError>,
{
    let split = front_matter::split_front_matter(content)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    let mut doc: Mapping = serde_yaml::from_str(split.yaml)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    f(&mut doc)?;
    let yaml = serde_yaml::to_string(&doc)
        .map_err(|err| CliError::new(err.to_string(), ExitStatus::Config))?;
    Ok(format!("---\n{}---\n{}", yaml, split.body))
}

fn current_sequence(doc: &Mapping, key: &str) -> Vec<Value> {
    doc.get(Value::from(key))
        .and_then(|value| value.as_sequence())
        .cloned()
        .unwrap_or_default()
}
