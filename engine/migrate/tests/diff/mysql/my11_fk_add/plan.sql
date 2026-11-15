alter table posts add constraint posts_user_fk foreign key (user_id) references users (id);
