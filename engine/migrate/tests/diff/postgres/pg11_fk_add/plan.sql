-- file 1 of 2
alter table posts add constraint posts_user_id_fkey foreign key (user_id) references users(id) not valid;
-- file 2 of 2
alter table posts validate constraint posts_user_id_fkey;
