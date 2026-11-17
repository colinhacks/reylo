create table users (id int not null primary key);
create table sessions (id uniqueidentifier not null primary key default newid(), user_id int not null references users (id) on delete cascade, expires datetimeoffset(3) not null);
