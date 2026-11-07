create table posts (
  id int not null,
  user_id int not null,
  title varchar(20) not null,
  primary key (id),
  key title (title),
  key user_id (user_id)
);
alter table posts add constraint posts_ibfk_1 foreign key (user_id) references users (id) on delete cascade;
create view titles as select p.id as id, p.title as title, u.id as uid from (posts p join users u on((u.id = p.user_id)));
