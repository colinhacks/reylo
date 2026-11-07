create table users (id serial primary key, name text not null);
create schema audit;
create table audit.audit_log (id bigint generated always as identity primary key, user_id integer references public.users, line text not null);
create index audit_log_user on audit.audit_log (user_id);
