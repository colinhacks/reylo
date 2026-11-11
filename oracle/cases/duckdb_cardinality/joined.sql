select a.handle, e.kind from accounts a join events e on e.account_id = a.id where a.id = :id
