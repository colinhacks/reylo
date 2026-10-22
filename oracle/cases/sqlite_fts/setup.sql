-- An external-content FTS5 table indexes the rows of another table, and a contentless one keeps
-- no rows at all: a row inserted straight into either leaves the index naming rows that are not
-- there ("fts5: missing row 1 from content table"), and every MATCH fails. The seeding fills the
-- content table and rebuilds the index from it, with or without the triggers that keep the two
-- in step, leaves a contentless table empty, and never writes to an FTS table's shadow tables.
create table recipes (id integer primary key, title text not null, body text);
create virtual table recipe_index using fts5(title, body, content='recipes', content_rowid='id');
create table articles (id integer primary key, title text, body text);
create virtual table article_index using fts5(title, body, content=articles, content_rowid=id);
create trigger articles_ai after insert on articles begin
  insert into article_index (rowid, title, body) values (new.id, new.title, new.body);
end;
create trigger articles_ad after delete on articles begin
  insert into article_index (article_index, rowid, title, body) values ('delete', old.id, old.title, old.body);
end;
create trigger articles_au after update on articles begin
  insert into article_index (article_index, rowid, title, body) values ('delete', old.id, old.title, old.body);
  insert into article_index (rowid, title, body) values (new.id, new.title, new.body);
end;
create virtual table words using fts5(word);
create virtual table bare using fts5(word, content='');
create virtual table notes using fts4(body);
