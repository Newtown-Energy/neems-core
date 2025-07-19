# User Roles

I need endpoints that let me get/set user profile information.  In the
User struct in @neems-core/src/models/user.rs and
@neems-core/src/models/user_role.rs, I see the email field, password,
institution, role, and totp_secret.  All of these eventually need
endpoints and workflows for managing them.
  
## Role

Some rules, in order of precedence:
   1 newtown-staff and newtown-admin roles are reserved for users who are at Newtown Energy 
  institution.
   2 Users with newtown-admin role can set any user's role to anything
   3 Users with newtown-staff role can set any user's role to anything except newtown-admin
   4 Users wtih admin role can set another user's role to admin if the target user is at the same 
  institution
   5 Users must have at least one role. The default role is user.

STATUS:

Need to consider adding the Newtown roles restriction trigger - this
would prevent data integrity issues even if application logic has bugs

## Email

A user setting a new email address on their own account should require
a confirmation email before it takes effect. A user setting a new
email on somebody else's account does not.

A user may set their own email address.  Admin can set email addresses
for anybody in their institution.  Newtown-staff can set email address
for anybody except Newtown-admin.  Newtown-admin can set email for
anybody.

STATUS: HOLD

We cannot implement this until we have implemented a way to
send email, which we have not done, so this feature is on hold for
now.  Do not implement.

## Password

Users can set their own password.

Admin can set password for anybody in their own institution.

Newtown-staff and newtown-admin can set password for anybody except newtown-admin

Newtown-admin can set password for anybody.

STATUS: TODO

## Institution

Rules:

 * Only settable by newtown* roles

If somebody has a Newtown Energy institution, they must have either newtown-staff or newtown-admin role

STATUS: TODO

## TOTP Secret

STATUS: HOLD

This feature is on hold. Do not implement.


# Standard Instructions
 * Be terse in reply
 * Be polite but not obsequious
 * Avoid assumptions. If unsure about anything, ask me
 * Instead of spelunking through the codebase, first ask where to look
