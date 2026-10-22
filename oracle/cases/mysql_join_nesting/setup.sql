-- How MySQL nests joins written without parentheses, which decides the tables an outer join
-- null-extends: a join with no ON of its own is the left operand of the joins after it, and
-- nests them only where an ON is left over for it; a comma binds looser than any JOIN. And
-- `->` binds tighter than any operator.
create table ta (a int not null);
create table tb (b int not null);
create table tc (c int not null);
create table tj (j json not null);
