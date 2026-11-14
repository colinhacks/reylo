create table users (id integer primary key, email text not null, n integer not null, updated_at text);
create index users_email_idx on users (email);
create trigger users_touch after update of email on users begin update users set updated_at = datetime('now') where id = new.id; end;
create view user_emails as select id, email from users;
create view user_emails_count as select count(*) as c from user_emails;
create table audit (id integer primary key, note text);
create trigger audit_users after insert on audit begin update users set n = n + 1 where id = new.id; end;
