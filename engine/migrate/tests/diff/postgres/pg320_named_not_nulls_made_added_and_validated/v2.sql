create table u (id int, b int constraint b_required not null default 0);
create table w (a int constraint a_required not null);
create table y (id int constraint y_id_named not null generated always as identity);
create table z (id int primary key, a int constraint z_a_named not null no inherit);
create table v (a int constraint v_a_new not null);
create table x (b int constraint x_b_named not null no inherit, c int generated always as identity constraint x_c_named not null);
