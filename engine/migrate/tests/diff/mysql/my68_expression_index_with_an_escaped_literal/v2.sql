create table users (first_name varchar(128) not null, last_name varchar(128) not null, index full_name ((concat(first_name, _utf8mb4'\'s first name'))));
