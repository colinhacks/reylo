create or replace algorithm = merge view some_view as select users.id as id from users with local check option;
