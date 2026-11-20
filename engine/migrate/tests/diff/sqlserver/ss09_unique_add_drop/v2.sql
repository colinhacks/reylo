create table accounts (id int not null primary key, email nvarchar(200) not null, handle nvarchar(40) not null, constraint accounts_handle unique (handle));
