-- A unique index under an operator class whose equality is finer than the `=` a comparison
-- resolves to holds rows that `=` finds equal, so that `=` pins no key of it. The built-in
-- `record_image_ops` compares bytes: it holds both (1.0) and (1.00) of a composite over numeric,
-- which record_eq, numeric's `=` per field, finds equal. An operator class of a user's over
-- numeric that compares the text of a value does the same.
create type pair as (a numeric);
create table t (id int primary key, c pair not null);
create unique index t_c_image on t (c record_image_ops);
insert into t values (1, '(1.0)'), (2, '(1.00)');
create table s (id int primary key, c pair);
insert into s values (1, '(1.0)');
create function num_text_cmp(numeric, numeric) returns int language sql immutable as $$ select case when $1::text < $2::text then -1 when $1::text > $2::text then 1 else 0 end $$;
create function num_text_lt(numeric, numeric) returns bool language sql immutable as $$ select $1::text < $2::text $$;
create function num_text_le(numeric, numeric) returns bool language sql immutable as $$ select $1::text <= $2::text $$;
create function num_text_eq(numeric, numeric) returns bool language sql immutable as $$ select $1::text = $2::text $$;
create function num_text_ge(numeric, numeric) returns bool language sql immutable as $$ select $1::text >= $2::text $$;
create function num_text_gt(numeric, numeric) returns bool language sql immutable as $$ select $1::text > $2::text $$;
create operator #< (leftarg = numeric, rightarg = numeric, function = num_text_lt);
create operator #<= (leftarg = numeric, rightarg = numeric, function = num_text_le);
create operator #= (leftarg = numeric, rightarg = numeric, function = num_text_eq);
create operator #>= (leftarg = numeric, rightarg = numeric, function = num_text_ge);
create operator #> (leftarg = numeric, rightarg = numeric, function = num_text_gt);
create operator class numeric_text_ops for type numeric using btree as
  operator 1 #<, operator 2 #<=, operator 3 #=, operator 4 #>=, operator 5 #>, function 1 num_text_cmp(numeric, numeric);
create table u (id int primary key, x numeric not null);
create unique index u_x_text on u (x numeric_text_ops);
insert into u values (1, 1.0), (2, 1.00);
-- A key under its type's default operator class beside them still pins.
create table v (id int primary key, x numeric not null unique);
