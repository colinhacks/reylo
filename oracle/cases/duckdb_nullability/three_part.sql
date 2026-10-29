-- A struct column named as its table is read by a three-part name, which is not a schema's.
select profiles.profiles.handle as handle, main.users.name as name from profiles, users where profiles.user_id = users.id
