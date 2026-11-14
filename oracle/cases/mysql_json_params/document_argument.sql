update profiles set doc = json_merge_patch(doc, :patch) where id = :id
