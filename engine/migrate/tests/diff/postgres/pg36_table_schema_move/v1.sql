create table users (id serial primary key, name text not null);
create table audit_log (id bigint generated always as identity primary key, user_id integer references users, line text not null);
create index audit_log_user on audit_log (user_id);
