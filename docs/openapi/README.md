# Accounting API Documentation

This directory contains API documentation for the accounting module.

## Files

- `openapi.yaml` - OpenAPI 3.0 specification (generated)
- Postman collections (generated after schema generation)

## Generating API Documentation

```bash
# Generate OpenAPI spec
backbone schema generate accounting --target openapi

# Generate Postman collection
backbone schema generate accounting --target postman
```

## API Base URL

- Development: `http://localhost:3000/api/v1`
- Production: Configure via `API_BASE_URL` environment variable

## Authentication

All endpoints require JWT authentication via Bearer token:

```
Authorization: Bearer <jwt_token>
```

## Common Response Formats

### Success Response

```json
{
  "success": true,
  "data": { ... },
  "meta": {
    "pagination": {
      "page": 1,
      "per_page": 20,
      "total": 100,
      "total_pages": 5
    }
  }
}
```

### Error Response

```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Validation failed",
    "details": [
      { "field": "email", "message": "Invalid email format" }
    ]
  }
}
```

## HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 201 | Created |
| 204 | No Content (successful delete) |
| 400 | Bad Request (validation error) |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 409 | Conflict (duplicate) |
| 422 | Unprocessable Entity |
| 500 | Internal Server Error |
