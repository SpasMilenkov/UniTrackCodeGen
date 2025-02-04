# UniTrackCodeGen - C# to TypeScript/Zod Generator

A code generation tool for converting C# enums and DTOs to TypeScript enums and Zod validation schemas, with support for documentation, localization, and customizable imports.

## Features

- **Type Conversion**

  - Convert C# enums to TypeScript enums with display names
  - Generate Zod schemas from C# DTOs
  - Support for nullable types, arrays, and complex types
  - Preserve XML documentation comments

- **Validation Support**

  - Generate Zod validation schemas
  - Support for standard C# validation attributes
  - Customizable error messages
  - Localized validation messages

- **Development Workflow**

  - File watching with automatic regeneration
  - Selective file processing
  - Custom ignore patterns
  - Maintains directory structure

- **Configuration**
  - Configurable file extensions
  - Custom import paths
  - Flexible i18n library integration
  - Structured import configuration

## Installation

```bash
# Clone the repository
git clone git@github.com:SpasMilenkov/UniTrackCodeGen.git
cd cs2ts

# Build the project
cargo build --release

# Optional: Install globally
cargo install --path .
```

## Usage

### Basic Commands

```bash
# Generate TypeScript enums
cs2ts enums -i ./src/enums -o ./src/generated

# Generate Zod schemas
cs2ts schemas -i ./src/dtos -o ./src/generated

# Watch mode
cs2ts enums -i ./src/enums -o ./src/generated --watch

# Generate localized schemas
cs2ts schemas -i ./src/dtos -o ./src/generated --localized
```

### Configuration

Create a `cs2ts.toml` file in your project root:

```toml
# File types to process
extensions = ["cs", "csx"]

# Paths to ignore (glob patterns)
ignore = [
    "**/bin/**",
    "**/obj/**",
    "**/*.generated.cs"
]

# Default directories
input_dir = "./src/backend"
output_dir = "./src/generated"

# Localization settings
localized = true
i18n_library = "@/i18n"  # Custom i18n library import path

# Additional imports configuration
[[additional_imports]]
name = "{ AdminRole }"
path = "@/enums/AdminRole"

[[additional_imports]]
name = "{ AdminStatus }"
path = "@/enums/AdminStatus"
```

## Input Examples

### C# Enum with Documentation

```csharp
/// <summary>
/// User permission types in the system
/// </summary>
public enum PermissionType
{
    /// <summary>
    /// Allows managing user accounts
    /// </summary>
    [Display(Name = "ManageUsers")]
    ManageUsers,

    /// <summary>
    /// Allows managing student records
    /// </summary>
    [Display(Name = "ManageStudents")]
    ManageStudents
}
```

### C# DTO with Validation

```csharp
/// <summary>
/// Data transfer object for login requests
/// </summary>
public record LoginDto(
    /// <summary>
    /// User's email address
    /// </summary>
    [Required(ErrorMessage = "Email is required.")]
    [EmailAddress(ErrorMessage = "Invalid email format.")]
    string Email,

    /// <summary>
    /// User's password
    /// </summary>
    [Required(ErrorMessage = "Password is required.")]
    [StringLength(100, MinimumLength = 8, ErrorMessage = "Password must be between 8 and 100 characters.")]
    string Password,

    /// <summary>
    /// Remember user's login
    /// </summary>
    bool RememberMe = false
);
```

## Output Examples

### TypeScript Enum

```typescript
/**
 * User permission types in the system
 */
export enum PermissionType {
  /** Allows managing user accounts */
  ManageUsers = "ManageUsers",

  /** Allows managing student records */
  ManageStudents = "ManageStudents",
}
```

### Zod Schema (Non-localized)

```typescript
import { z } from "zod";

/**
 * Data transfer object for login requests
 */
export const LoginDtoSchema = z.object({
  /** User's email address */
  email: z
    .string()
    .required("Email is required.")
    .email("Invalid email format."),

  /** User's password */
  password: z
    .string()
    .required("Password is required.")
    .min(8, "Password must be between 8 and 100 characters.")
    .max(100, "Password must be between 8 and 100 characters."),

  /** Remember user's login */
  rememberMe: z.boolean().default(false),
});

export type LoginDto = z.infer<typeof LoginDtoSchema>;
```

### Zod Schema (Localized)

```typescript
import { z } from "zod";
import { useI18n } from "@/i18n";

/**
 * Data transfer object for login requests
 */
export const LoginDtoSchema = () => {
  const { t } = useI18n();

  return z.object({
    /** User's email address */
    email: z
      .string({
        required_error: t("login.validation.email.required"),
      })
      .email({
        message: t("login.validation.email.invalid"),
      }),

    /** User's password */
    password: z
      .string({
        required_error: t("login.validation.password.required"),
      })
      .min(8, {
        message: t("login.validation.password.minLength"),
      })
      .max(100, {
        message: t("login.validation.password.maxLength"),
      }),

    /** Remember user's login */
    rememberMe: z.boolean().default(false),
  });
};

export type LoginDto = z.infer<typeof LoginDtoSchema>;
```

## Features in Detail

### Documentation

- Preserves C# XML documentation comments
- Supports `<summary>`, `<remarks>`, and `<example>` tags
- Carries over documentation to generated TypeScript/Zod files

### Type Conversion

- Handles all common C# types
- Supports nullable types (`string?`, `int?`, etc.)
- Converts C# arrays and collections to TypeScript arrays
- Handles complex types and nested objects

### Validation

- Supports common C# validation attributes:
  - `[Required]`
  - `[StringLength]`
  - `[EmailAddress]`
  - `[Range]`
  - `[RegularExpression]`
  - Custom validation messages

### Localization

- Configurable i18n library integration
- Supports different localization patterns
- Customizable message paths
- Optional localization support

### Development Workflow

- Watch mode for automatic regeneration
- Preserves source directory structure
- Selective file processing with glob patterns
- Detailed console output with statistics

## Contributing

Contributions are welcome! Please feel free to submit pull requests.

## License

MIT License - see LICENSE file for details

## Cover art courtesy to

<a href="https://www.flaticon.com/free-icons/translate" title="translate icons">Translate icons created by Soodesign - Flaticon</a>
