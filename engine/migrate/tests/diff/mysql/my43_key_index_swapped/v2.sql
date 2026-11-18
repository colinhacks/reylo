create table users (id int primary key);
create table accounts (user_id int not null, id int not null, primary key (user_id, id));
create table transactions (id int primary key, user_id int not null, account_id int not null, constraint transactions_user_fk foreign key (user_id) references users (id), constraint transactions_account_fk foreign key (user_id, account_id) references accounts (user_id, id));
