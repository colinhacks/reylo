create table messages (id integer primary key, room text not null, body text not null);
create table rooms (id integer primary key, name text not null);
create publication live for table messages;
