### Admin User Initialization Fairing: Implementation Plan

#### Overview

You have provided all the necessary details to proceed. Here’s a concise, actionable breakdown for implementing a Rocket fairing that ensures a default admin user and role for "Newtown Energy" are present at startup.

#### Steps

1. **Fairing Setup**
   - Implement a Rocket fairing that runs during startup.
   - The fairing must have access to a database connection and block launch on any failure.

DONE.

2. **Institution Lookup/Creation**
   - Search for an institution named:
     - "Newtown Energy"
     - "Newtown Energy, Inc"
     - "Newtown Energy, Inc."
   - If none are found, create an institution named "Newtown Energy".

DONE.

3. **Role Lookup/Creation**
   - Check if a role named `newtown-admin` exists for the institution.
   - If not, create the `newtown-admin` role and associate it with the institution.

DONE.

4. **Admin User Existence Check**
   - Query for any user with the `newtown-admin` role.
   - If such a user exists, the process ends here.

DONE.

5. **Environment Variable Handling**
   - Use dotenv to load environment variables.
   - Email: Use `NEEMS_DEFAULT_USER` or default to `admin@example.com`.
   - Password: Use `NEEMS_DEFAULT_PASSWORD` or default to `admin`.

DONE.

6. **User Creation**
   - Use the existing user insertion function (which handles password hashing).
   - Do not set a TOTP secret.
   - Assign the new user the `newtown-admin` role for the institution.

DONE.

7. **Error Handling**
   - On any error (DB, creation, missing prerequisites), print a clear error message and terminate the process.

DONE.

8. **Testing**
   - Write a unit or integration test to verify:
     - The admin user is created if missing.
     - No duplicate admin user is created if one already exists.
     - The `newtown-admin` role is created if missing and not duplicated.

DONE.

#### Notes

- The fairing should be attached in both test and production environments as needed.
- No feature flags are required for this logic.
- The process should be idempotent: running it multiple times must not create duplicates.
- All error situations must cause a startup failure with a clear log message.

