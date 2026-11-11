insert into accounts (id, status, tier, m, legacy) values (:id, :status, :tier, :m, :legacy) on conflict (id) do update set status = :new_status returning status
