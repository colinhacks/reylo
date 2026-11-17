if @flag = 1
begin
  declare @n int = @id
  select id, note from orders where id = @n
end
