insert into users (name) values ('Ada'), ('Bob');
insert into audit_log (user_id, line) values (1, 'signed in'), (2, 'signed in'), (1, 'signed out');
