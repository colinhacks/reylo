create policy users_self on users for select using (id = 1);
alter table users enable row level security;
