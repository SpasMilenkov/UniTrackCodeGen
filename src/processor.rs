use crate::config::Config;
use chrono::Local;
use colored::*;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

lazy_static! {
    static ref ENUM_REGEX: Regex =
        Regex::new(r"public\s+enum\s+(?P<name>\w+)\s*\{(?P<body>[^}]+)\}").unwrap();
    static ref DISPLAY_ATTR_REGEX: Regex =
        Regex::new(r#"\[Display\(Name\s*=\s*"([^"]+)"\)\]"#).unwrap();
    static ref DTO_REGEX: Regex =
        Regex::new(r"public\s+record\s+(?P<name>\w+)\s*\((?P<props>[^)]+)\)").unwrap();
    static ref PROPERTY_REGEX: Regex =
        Regex::new(r"(?m)(?P<type>[a-zA-Z0-9_<>?\[\]\.]+)\s+(?P<name>[a-zA-Z0-9_]+)(?:\s*,|\s*$)")
            .unwrap();
    static ref VALIDATION_REGEX: Regex = Regex::new(r"\[(?P<attr>[^\]]+)\]").unwrap();
    static ref DOC_COMMENT_REGEX: Regex = Regex::new(r"///\s*<(?:summary|remarks|example)>(.*?)</(?:summary|remarks|example)>").unwrap();
    static ref PROP_DOC_REGEX: Regex = 
        Regex::new(r#"(?m)^\s*///\s*<(?:summary|remarks|example)>(.*?)</(?:summary|remarks|example)>\s*(?:[^\n]*\n)*\s*(?P<type>[a-zA-Z0-9_<>?\[\]\.]+)\s+(?P<name>[a-zA-Z0-9_]+)"#).unwrap();
}

#[derive(Debug, Default)]
pub struct ProcessingStats {
    pub files_processed: usize,
    pub enums_generated: usize,
    pub schemas_generated: usize,
    pub files_skipped: usize,
}

impl ProcessingStats {
    pub fn print_summary(&self) {
        println!("\n📊 Generation Summary:");
        println!(
            "├─ Files processed: {}",
            self.files_processed.to_string().cyan()
        );
        println!(
            "├─ Enums generated: {}",
            self.enums_generated.to_string().green()
        );
        println!(
            "├─ Schemas generated: {}",
            self.schemas_generated.to_string().green()
        );
        println!(
            "└─ Files skipped: {}",
            self.files_skipped.to_string().yellow()
        );
    }
}

#[derive(Debug)]
pub struct FileProcessor {
    file_hashes: HashMap<PathBuf, u64>,
    file_mapping: HashMap<PathBuf, Vec<PathBuf>>,
    pub stats: ProcessingStats,
}

#[derive(Debug)]
enum CSharpType {
    String,
    Int,
    Double,
    Decimal,
    Bool,
    DateTime,
    Guid,
    Array(Box<CSharpType>),
    Nullable(Box<CSharpType>),
    Dictionary(Box<CSharpType>, Box<CSharpType>),
    Custom(String),
}

#[derive(Debug)]
struct EnumValue {
    name: String,
    display_name: Option<String>,
    documentation: Option<String>,
}

#[derive(Debug)]
struct CSharpEnum {
    name: String,
    values: Vec<EnumValue>,
    documentation: Option<String>,
}

#[derive(Debug)]
struct ValidationRule {
    rule_type: String,
    parameters: HashMap<String, String>,
    error_message: Option<String>,
    condition: Option<String>,
}

#[derive(Debug)]
struct DtoProperty {
    name: String,
    type_name: CSharpType,
    validations: Vec<ValidationRule>,
    documentation: Option<String>,
}

#[derive(Debug)]
struct CSharpDto {
    name: String,
    properties: Vec<DtoProperty>,
    documentation: Option<String>,
}

fn generate_file_header(config: &Config, file_type: &str) -> String {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut header = String::new();

    header.push_str("/**\n");
    header.push_str(&format!(
        " * Generated with UniTrackCodeGen at {}\n",
        timestamp
    ));
    header.push_str(" * \n");
    header.push_str(" * Configuration:\n");
    header.push_str(&format!(" * - File Type: {}\n", file_type));
    header.push_str(&format!(
        " * - Input Directory: {}\n",
        config
            .input_dir
            .as_ref()
            .map_or("default", |p| p.to_str().unwrap_or("invalid"))
    ));
    header.push_str(&format!(
        " * - Output Directory: {}\n",
        config
            .output_dir
            .as_ref()
            .map_or("default", |p| p.to_str().unwrap_or("invalid"))
    ));
    header.push_str(&format!(
        " * - Extensions: [{}]\n",
        config.extensions.join(", ")
    ));
    header.push_str(&format!(
        " * - Localization: {}\n",
        if config.localized {
            "enabled"
        } else {
            "disabled"
        }
    ));
    header.push_str(" */\n\n");

    header
}

impl FileProcessor {
    pub fn new() -> Self {
        Self {
            file_hashes: HashMap::new(),
            file_mapping: HashMap::new(),
            stats: ProcessingStats::default(),
        }
    }

    pub fn should_process_file(&mut self, path: &Path) -> bool {
        let content = match std::fs::read(path) {
            Ok(content) => content,
            Err(_) => return false,
        };

        let hash = seahash::hash(&content);
        let path = path.to_path_buf();

        if let Some(&old_hash) = self.file_hashes.get(&path) {
            if old_hash == hash {
                return false;
            }
        }

        self.file_hashes.insert(path, hash);
        true
    }

    pub fn register_output(&mut self, input: PathBuf, output: PathBuf) {
        self.file_mapping
            .entry(input)
            .or_insert_with(Vec::new)
            .push(output);
    }

    pub fn get_outputs_for_input(&self, input: &Path) -> Option<&Vec<PathBuf>> {
        self.file_mapping.get(&input.to_path_buf())
    }

    pub fn cleanup_outputs(&self, input: &Path) -> std::io::Result<()> {
        if let Some(outputs) = self.get_outputs_for_input(input) {
            for output in outputs {
                if output.exists() {
                    std::fs::remove_file(output)?;
                }
            }
        }
        Ok(())
    }

    fn get_relative_output_path(
        &self,
        input_path: &Path,
        input_root: &Path,
        output_root: &Path,
    ) -> PathBuf {
        let relative = input_path.strip_prefix(input_root).unwrap_or(input_path);
        output_root.join(relative)
    }

    pub fn process_file(
        &mut self,
        input_path: &Path,
        input_root: &Path,
        output_root: &Path,
        config: &Config,
    ) -> std::io::Result<()> {
        if !self.should_process_file(input_path) {
            self.stats.files_skipped += 1;
            return Ok(());
        }

        self.stats.files_processed += 1;
        self.cleanup_outputs(input_path)?;

        let content = std::fs::read_to_string(input_path)?;

        // Process enums
        if let Ok(enums) = CSharpEnum::parse(&content) {
            for enum_def in enums {
                let relative_path =
                    self.get_relative_output_path(input_path, input_root, output_root);
                let output_dir = relative_path.parent().unwrap_or(output_root);
                std::fs::create_dir_all(output_dir)?;

                let output_path = output_dir.join(
                    input_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .replace(".cs", ".ts"),
                );
                std::fs::write(&output_path, enum_def.to_typescript())?;
                self.register_output(input_path.to_path_buf(), output_path);
                self.stats.enums_generated += 1;
            }
        }

        // Process DTOs
        if let Ok(dtos) = CSharpDto::parse(&content) {
            for dto in dtos {
                let relative_path =
                    self.get_relative_output_path(input_path, input_root, output_root);
                let output_dir = relative_path.parent().unwrap_or(output_root);
                std::fs::create_dir_all(output_dir)?;

                let output_path = output_dir.join(
                    input_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .replace(".cs", ".schema.ts"),
                );
                std::fs::write(&output_path, dto.to_zod_schema(&config))?;
                self.register_output(input_path.to_path_buf(), output_path);
                self.stats.schemas_generated += 1;
            }
        }

        Ok(())
    }
}

impl CSharpType {
    fn from_string(type_str: &str) -> Self {
        match type_str {
            "string" => CSharpType::String,
            "int" | "Int32" => CSharpType::Int,
            "double" | "Double" => CSharpType::Double,
            "decimal" | "Decimal" => CSharpType::Decimal,
            "bool" | "Boolean" => CSharpType::Bool,
            "DateTime" => CSharpType::DateTime,
            "Guid" => CSharpType::Guid,
            s if s.starts_with("List<") || s.starts_with("IEnumerable<") => {
                let inner = s[s.find('<').unwrap() + 1..s.find('>').unwrap()].trim();
                CSharpType::Array(Box::new(CSharpType::from_string(inner)))
            }
            s if s.starts_with("Dictionary<") => {
                let content = &s[s.find('<').unwrap() + 1..s.find('>').unwrap()];
                let mut parts = content.split(',');
                let key = parts.next().unwrap().trim();
                let value = parts.next().unwrap().trim();
                CSharpType::Dictionary(
                    Box::new(CSharpType::from_string(key)),
                    Box::new(CSharpType::from_string(value)),
                )
            }
            s if s.ends_with('?') => {
                let base_type = &s[..s.len() - 1];
                CSharpType::Nullable(Box::new(CSharpType::from_string(base_type)))
            }
            s => CSharpType::Custom(s.to_string()),
        }
    }

    fn to_zod_type(&self, localized: bool, is_update_dto: bool) -> String {
        let base_type = match self {
            CSharpType::String => "z.string()".to_string(),
            CSharpType::Int => "z.number().int()".to_string(),
            CSharpType::Double | CSharpType::Decimal => "z.number()".to_string(),
            CSharpType::Bool => "z.boolean()".to_string(),
            CSharpType::Guid => "z.string().uuid()".to_string(),
            CSharpType::DateTime => {
                if localized {
                    "z.date().or(z.string().datetime())".to_string()
                } else {
                    "z.string().datetime()".to_string()
                }
            }
            CSharpType::Array(inner) => {
                format!("z.array({})", inner.to_zod_type(localized, is_update_dto))
            }
            CSharpType::Nullable(inner) => {
                format!("{}.nullable()", inner.to_zod_type(localized, is_update_dto))
            }
            CSharpType::Dictionary(key, value) => format!(
                "z.record({}, {})",
                key.to_zod_type(localized, is_update_dto),
                value.to_zod_type(localized, is_update_dto)
            ),
            CSharpType::Custom(name) => format!("{}Schema", name),
        };

        // Make all fields required by default for create DTOs, optional for update DTOs
        if is_update_dto {
            format!("{}.optional()", base_type)
        } else {
            format!("{}.required()", base_type)
        }
    }
}

impl ValidationRule {
    fn to_zod_validation(&self, prop_name: &str, localized: bool) -> Option<String> {
        match self.rule_type.as_str() {
            "Required" => Some(".required()".to_string()),
            "Range" => {
                let min = self.parameters.get("Minimum")?;
                let max = self.parameters.get("Maximum")?;
                let error_msg = if localized {
                    format!("t('{}.range')", prop_name)
                } else {
                    self.error_message
                        .clone()
                        .unwrap_or_else(|| format!("Value must be between {} and {}", min, max))
                };
                Some(format!(
                    ".min({}, {{ message: {} }}).max({}, {{ message: {} }})",
                    min, error_msg, max, error_msg
                ))
            }
            "StringLength" => {
                let mut validation = String::new();
                if let Some(min) = self.parameters.get("MinimumLength") {
                    let error_msg = if localized {
                        format!("t('{}.minLength')", prop_name)
                    } else {
                        self.error_message
                            .clone()
                            .unwrap_or_else(|| format!("Minimum length is {}", min))
                    };
                    validation.push_str(&format!(".min({}, {{ message: {} }})", min, error_msg));
                }
                if let Some(max) = self.parameters.get("MaximumLength") {
                    let error_msg = if localized {
                        format!("t('{}.maxLength')", prop_name)
                    } else {
                        self.error_message
                            .clone()
                            .unwrap_or_else(|| format!("Maximum length is {}", max))
                    };
                    validation.push_str(&format!(".max({}, {{ message: {} }})", max, error_msg));
                }
                Some(validation)
            }
            "EmailAddress" => {
                let error_msg = if localized {
                    format!("t('{}.email')", prop_name)
                } else {
                    self.error_message
                        .clone()
                        .unwrap_or_else(|| "Invalid email address".to_string())
                };
                Some(format!(".email({{ message: {} }})", error_msg))
            }
            "Phone" => {
                let error_msg = if localized {
                    format!("t('{}.phone')", prop_name)
                } else {
                    self.error_message
                        .clone()
                        .unwrap_or_else(|| "Invalid phone number".to_string())
                };
                Some(format!(
                    ".regex(/^\\+?[1-9]\\d{{1,14}}$/, {{ message: {} }})",
                    error_msg
                ))
            }
            "RegularExpression" => {
                if let Some(pattern) = self.parameters.get("pattern") {
                    let error_msg = if localized {
                        format!("t('{}.pattern')", prop_name)
                    } else {
                        self.error_message
                            .clone()
                            .unwrap_or_else(|| "Invalid format".to_string())
                    };
                    Some(format!(
                        ".regex(new RegExp('{}'), {{ message: {} }})",
                        pattern, error_msg
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl CSharpEnum {
    fn parse(content: &str) -> Result<Vec<Self>, &'static str> {
        let mut enums = Vec::new();

        // Extract documentation if present
        let get_documentation = |text: &str| -> Option<String> {
            DOC_COMMENT_REGEX
                .captures(text)
                .map(|cap| cap[1].trim().to_string())
        };

        for enum_match in ENUM_REGEX.captures_iter(content) {
            let name = enum_match.name("name").unwrap().as_str().to_string();
            let body = enum_match.name("body").unwrap().as_str();

            // Get documentation before the enum
            let documentation = get_documentation(
                &content[..enum_match.get(0).unwrap().start()]
                    .lines()
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );

            let values = body
                .split(',')
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .map(|line| {
                    let display_name = DISPLAY_ATTR_REGEX
                        .captures(line)
                        .map(|cap| cap[1].to_string());

                    let name = line.split_whitespace().last().unwrap().to_string();

                    // Get documentation for enum value
                    let documentation = get_documentation(line);

                    EnumValue {
                        name,
                        display_name,
                        documentation,
                    }
                })
                .collect();

            enums.push(Self {
                name,
                values,
                documentation,
            });
        }

        Ok(enums)
    }

    fn to_typescript(&self) -> String {
        let mut output = String::new();

        // Add generated header
        output.push_str(&generate_file_header(
            &Config::load().unwrap_or_default(),
            "Enum",
        ));

        // Rest of the implementation remains the same...
        if let Some(doc) = &self.documentation {
            output.push_str("/**\n");
            output.push_str(&format!(" * {}\n", doc));
            output.push_str(" */\n");
        }

        output.push_str(&format!("export enum {} {{\n", self.name));

        for value in &self.values {
            // Add documentation for enum value if present
            if let Some(doc) = &value.documentation {
                output.push_str(&format!("  /** {} */\n", doc));
            }

            output.push_str(&format!(
                "  {} = '{}',\n",
                value.name,
                value.display_name.as_ref().unwrap_or(&value.name)
            ));
        }

        output.push_str("}\n");
        output
    }
}

impl CSharpDto {
    fn parse(content: &str) -> Result<Vec<Self>, &'static str> {
        let mut dtos = Vec::new();

        for dto_match in DTO_REGEX.captures_iter(content) {
            let name = dto_match.name("name").unwrap().as_str().to_string();
            let props_str = dto_match.name("props").unwrap().as_str();
            
            // Get all documentation comments before the DTO definition
            let documentation = DOC_COMMENT_REGEX
                .captures_iter(&content[..dto_match.get(0).unwrap().start()])
                .map(|cap| cap[1].trim().to_string())
                .collect::<Vec<_>>()
                .join("\n");

            let documentation = if documentation.is_empty() {
                None
            } else {
                Some(documentation)
            };

            let mut properties = Vec::new();

            // Process properties with their documentation
            for prop in props_str.split(',') {
                if let Some(cap) = PROPERTY_REGEX.captures(prop.trim()) {
                    let type_str = cap.name("type").unwrap().as_str().trim();
                    let name = cap.name("name").unwrap().as_str().trim().to_string();
                    let type_name = CSharpType::from_string(type_str);

                    let prop_docs = DOC_COMMENT_REGEX
                        .captures_iter(prop)
                        .map(|cap| cap[1].trim().to_string())
                        .collect::<Vec<_>>()
                        .join("\n");

                    properties.push(DtoProperty {
                        name,
                        type_name,
                        validations: Vec::new(),
                        documentation: if prop_docs.is_empty() { None } else { Some(prop_docs) },
                    });
                }
            }

            dtos.push(Self {
                name,
                properties,
                documentation,
            });
        }

        Ok(dtos)
    }

    fn is_update_dto(&self) -> bool {
        self.name.starts_with("Update")
    }

    fn to_zod_schema(&self, config: &Config) -> String {
        let mut output = String::new();
        output.push_str(&generate_file_header(config, "Zod Schema"));

        // Add imports
        output.push_str("import { z } from 'zod';\n");
        
        // i18n import if localized
        if config.localized {
            output.push_str(&format!("import {{ useI18n }} from '{}';\n", config.i18n_library));
        }
        
        // Additional imports
        for import in &config.additional_imports {
            output.push_str(&format!("import {} from '{}';\n", import.name, import.path));
        }
        
        output.push_str("\n");

        let is_update = self.is_update_dto();

        // Add documentation if available
        if let Some(doc) = &self.documentation {
            output.push_str("/**\n");
            output.push_str(&format!(" * {}\n", doc));
            output.push_str(" */\n");
        }

        if config.localized {
            output.push_str(&format!("export const {}Schema = () => {{\n", self.name));
            output.push_str("  const { t } = useI18n();\n");
            output.push_str("  return z.object({\n");
        } else {
            output.push_str(&format!("export const {}Schema = z.object({{\n", self.name));
        }

        // Generate properties
        for prop in &self.properties {
            if let Some(doc) = &prop.documentation {
                output.push_str(&format!("    /** {} */\n", doc));
            }

            let mut schema_line = format!(
                "    {}: {}",
                prop.name,
                prop.type_name.to_zod_type(config.localized, is_update)
            );

            // Add additional validations (but don't add .required() since it's already handled)
            for validation in &prop.validations {
                if validation.rule_type != "Required" {
                    if let Some(validation_code) =
                        validation.to_zod_validation(&prop.name, config.localized)
                    {
                        schema_line.push_str(&validation_code);
                    }
                }
            }

            output.push_str(&schema_line);
            output.push_str(",\n");
        }

        if config.localized {
            output.push_str("  });\n};\n");
        } else {
            output.push_str("});\n");
        }

        output.push_str(&format!(
            "\nexport type {} = z.infer<typeof {}Schema>;\n",
            self.name, self.name
        ));

        output
    }
}

pub fn process_directory(
    processor: &mut FileProcessor,
    dir_path: &Path,
    input_root: &Path,
    output_root: &Path,
    config: &Config,
) -> std::io::Result<()> {
    if dir_path.is_dir() {
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                process_directory(processor, &path, input_root, output_root, config)?;
            } else if config.is_valid_extension(&path.to_path_buf())
                && !config.should_ignore(&path.to_path_buf())
            {
                processor.process_file(&path, input_root, output_root, config)?;
            }
        }
    } else if config.is_valid_extension(&dir_path.to_path_buf())
        && !config.should_ignore(&dir_path.to_path_buf())
    {
        processor.process_file(dir_path, input_root, output_root, config)?;
    }

    Ok(())
}

pub fn process_single_file(
    processor: &mut FileProcessor,
    input_path: &Path,
    output_dir: &Path,
    config: &Config,
) -> std::io::Result<()> {
    let input_root = config
        .input_dir
        .as_ref()
        .map(|p| p.as_path())
        .unwrap_or_else(|| input_path.parent().unwrap_or(Path::new("")));

    if input_path.is_dir() {
        process_directory(processor, input_path, input_root, output_dir, config)
    } else {
        processor.process_file(input_path, input_root, output_dir, config)
    }
}
