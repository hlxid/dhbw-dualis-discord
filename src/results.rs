use std::{fs::File, io::BufReader, path::Path};
use serde::{Deserialize, Serialize};

const FILE_PATH: &str = "./dualis_results.json";

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CourseResult {
    pub course_id: String,
    pub course_name: String,
    pub scored: bool,
}

impl CourseResult {
    pub fn new(course_id: String, course_name: String, scored: bool) -> Self {
        Self {
            course_id,
            course_name,
            scored,
        }
    }
}

pub fn load_results() -> Option<Vec<CourseResult>> {
    let path = Path::new(FILE_PATH);
    if !path.exists() {
        return None;
    }

    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let results: Vec<CourseResult> = serde_json::from_reader(reader).unwrap();

    println!("Successfully loaded {} results!", results.len());
    Some(results)
}

pub fn save_results(results: &[CourseResult]) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(&results)?;
    std::fs::write(FILE_PATH, json)?;
    println!("Successfully saved results to {FILE_PATH}.");

    Ok(())
}

pub fn diff_results<'a>(old: &[CourseResult], new: &'a [CourseResult]) -> Vec<&'a CourseResult> {
    println!("Looking for newly scored courses...");
    let mut changed = vec![];

    for entry in new {
        let old_entry = old
            .iter()
            .find(|old_entry| old_entry.course_id == entry.course_id);
        if old_entry.is_none() {
            continue;
        }

        if entry.scored && !old_entry.unwrap().scored {
            changed.push(entry);
        }
    }

    println!("Found {} newly scored courses!", changed.len());
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_results() {
        let old = vec![
            CourseResult::new("1".to_string(), "Test".to_string(), false),
            CourseResult::new("2".to_string(), "Test".to_string(), false),
            CourseResult::new("3".to_string(), "Test".to_string(), false),
            CourseResult::new("4".to_string(), "Test".to_string(), false),
            CourseResult::new("5".to_string(), "Test".to_string(), false),
        ];
        let new = vec![
            CourseResult::new("1".to_string(), "Test".to_string(), true),
            CourseResult::new("2".to_string(), "Test".to_string(), false),
            CourseResult::new("3".to_string(), "Test".to_string(), false),
            CourseResult::new("4".to_string(), "Test".to_string(), false),
            CourseResult::new("5".to_string(), "Test".to_string(), false),
        ];
        let changed = diff_results(&old, &new);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], &CourseResult::new("1".to_string(), "Test".to_string(), true));
    }
}

