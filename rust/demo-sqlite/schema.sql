create table users (
  id integer primary key,
  name text not null,
  nickname text,
  score real not null default 0,
  active boolean not null default 1,
  visits bigint not null default 0,
  avatar blob
);
create table posts (
  id integer primary key,
  author_id integer not null references users (id),
  title text not null
);
insert into users (id, name, nickname, score, active, visits, avatar) values (1, 'ada', null, 9.5, 1, 12, x'00ff'), (2, 'bob', 'bobby', 3.25, 0, 3, null);
insert into posts (author_id, title) values (1, 'hello'), (1, 'again');
