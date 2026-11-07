alter table posts add constraint posts_user_id_fkey foreign key (user_id) references users(id);
