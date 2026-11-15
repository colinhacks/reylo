alter table posts drop foreign key posts_user_fk, drop index posts_user_fk;
alter table posts modify column user_id bigint not null;
alter table users modify column id bigint not null;
alter table posts add constraint posts_user_fk foreign key (user_id) references users (id);
