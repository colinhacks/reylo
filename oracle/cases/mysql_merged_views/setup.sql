-- A view's column read on the null-extended side of an outer join is NULL where the join finds no
-- row. The server merges a view into the query that reads it, and a merged expression keeps its
-- own NOT NULL flag there; a view created with ALGORITHM = MERGE is merged whatever an optimizer
-- hint says, so the engine reads every view through a derived table, which is materialized.
create table users (id int auto_increment primary key, name text not null, email text);
create table posts (id int auto_increment primary key, user_id int not null, title text not null);
create algorithm = merge view merged as select id, user_id, id + 1 as next, upper(title) as shout from posts;
create view plain as select id, user_id, id + 1 as next from posts;
create algorithm = temptable view temp as select id, user_id, id + 1 as next from posts;
