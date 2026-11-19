grant select on table u to current_user;
create table fresh (
  id integer
);
revoke delete on table t from current_user;
revoke insert on table fresh from current_user;
revoke insert on table t from current_user;
revoke maintain on table t from current_user;
revoke references on table t from current_user;
revoke select on table t from current_user;
revoke trigger on table t from current_user;
revoke truncate on table t from current_user;
revoke update on table fresh from current_user;
revoke update on table t from current_user;
revoke usage on sequence s from current_user;
