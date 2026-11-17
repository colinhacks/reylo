insert into orders (total, placed_at) values (@total, sysutcdatetime())
select cast(scope_identity() as int) as id
