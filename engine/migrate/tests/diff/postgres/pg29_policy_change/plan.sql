drop policy docs_read on docs;
create policy docs_read on docs for select to reader, writer using (owner_name = current_user or body is null);
