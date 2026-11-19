-- loses data: lab_results.flash_summary
alter publication lab_feed set table only lab_results (id);
alter table lab_results drop column flash_summary;
