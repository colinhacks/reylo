create table users (
  id bigint unsigned primary key,
  name varchar(50) not null,
  nickname varchar(50),
  active boolean not null default true,
  status enum('new', 'gone') not null default 'new',
  joined timestamp(3) not null default current_timestamp(3),
  born date,
  span time,
  balance decimal(10, 2) not null default 0,
  since year,
  flags bit(8),
  avatar blob
);
create table posts (
  id int unsigned primary key auto_increment,
  author_id bigint unsigned not null,
  title varchar(100) not null,
  foreign key (author_id) references users (id)
);
-- A JSON column a CHECK holds to a JSON Schema, which types it field by field.
create table settings (
  user_id bigint unsigned primary key,
  prefs json not null,
  check (json_schema_valid('{"type":"object","properties":{"theme":{"enum":["dark","light"]},"font_size":{"type":"integer"},"tabs":{"type":"array","items":{"type":"string"}}},"required":["theme","font_size"],"additionalProperties":false}', prefs)),
  foreign key (user_id) references users (id)
);
insert into users (id, name, nickname, born, span, since, flags, avatar) values (1, 'ada', 'countess', '1815-12-10', '-838:59:59', 1901, b'10100101', x'00ff'), (2, 'bob', null, null, null, null, null, null);
insert into posts (author_id, title) values (1, 'hello'), (1, 'again');
insert into settings values (1, '{"theme": "dark", "font_size": 14, "tabs": ["a"]}'), (2, '{"theme": "light", "font_size": 12}');
