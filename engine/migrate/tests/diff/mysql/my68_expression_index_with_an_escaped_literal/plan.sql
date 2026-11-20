alter table users
  drop index full_name,
  add key full_name ((concat(first_name, _utf8mb4'\'s first name')));
