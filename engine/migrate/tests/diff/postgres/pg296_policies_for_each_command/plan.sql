create policy d on t for delete using (owner = current_user);
create policy i on t for insert with check (owner = current_user);
create policy r on t as restrictive for all using (id > 0);
create policy s on t for select using (owner = current_user);
create policy u on t for update using (owner = current_user);
