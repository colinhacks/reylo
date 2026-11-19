create schema app;
create table app.downloads (id bigint primary key);
create table app.documents (id bigint primary key, title text not null);
