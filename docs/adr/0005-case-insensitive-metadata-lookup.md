# Preserve metadata keys but match them case-insensitively

`anchor` keeps metadata keys exactly as the user wrote them, but user-facing lookups match case-insensitively and fail on ambiguity when two keys differ only by case. We chose this because it keeps the file format free-form and portable while avoiding brittle exact-case lookups for common fields like `url` or `URL`.
