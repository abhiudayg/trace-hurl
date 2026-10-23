# Scenario: User Management & Authentication Verification Suite
# Verifies user creation, profile retrieval with contract assertions,
# error handling for missing resources, and health checks.

POST http://localhost:8080/api/v1/users
Content-Type: application/json
Authorization: Bearer secret-admin-token
{
  "name": "Jane Doe",
  "email": "jane.doe@example.com",
  "role": "developer"
}
HTTP 201
[Asserts]
status == 201
header "Content-Type" contains "application/json"
jsonpath "$.status" == "created"
jsonpath "$.user.id" matches "^usr_[a-zA-Z0-9]+$"
jsonpath "$.user.email" == "jane.doe@example.com"
[Captures]
user_id: jsonpath "$.user.id"

GET http://localhost:8080/api/v1/users/{{user_id}}
Accept: application/json
Authorization: Bearer secret-admin-token
HTTP 200
[Asserts]
status == 200
jsonpath "$.id" == "{{user_id}}"
jsonpath "$.name" == "Jane Doe"
jsonpath "$.email" == "jane.doe@example.com"
jsonpath "$.role" == "developer"
jsonpath "$.authCode" != null

GET http://localhost:8080/api/v1/users/usr_nonexistent
Accept: application/json
Authorization: Bearer secret-admin-token
HTTP 404
[Asserts]
status == 404
jsonpath "$.error" == "UserNotFound"

GET http://localhost:8080/api/v1/health
HTTP 200
[Asserts]
status >= 200
