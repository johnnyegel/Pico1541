import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

# Update DiskTask struct
old_struct = """pub struct DiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
    track_handle: ManagerHandle,
    opener: O,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
}"""

new_struct = """pub struct DiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
    track_handle: ManagerHandle,
    opener: O,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    pre_cached_ht: Option<(u8, pico1541_common::track_buffer::TrackLayout, usize, Option<u8>)>,
    last_step_direction: i8,
}"""
content = content.replace(old_struct, new_struct)

# Update DiskTask::new
old_new = """        Self {
            disk_image,
            track_handle,
            opener,
            mounted_dir,
        }"""
new_new = """        Self {
            disk_image,
            track_handle,
            opener,
            mounted_dir,
            pre_cached_ht: None,
            last_step_direction: 2,
        }"""
content = content.replace(old_new, new_new)

# Insert cache_adjacent_tracks
new_method = """
    async fn cache_adjacent_tracks(&mut self) -> Option<u8> {
        let current_ht = self.track_handle.get_head_half_track();
        
        let desired_track = (current_ht as i16 + self.last_step_direction as i16) as u8;
        if desired_track == 0 || desired_track > 84 {
            return None;
        }

        if let Some((ht, _, _, _)) = self.pre_cached_ht {
            if ht == desired_track {
                return None; // Already cached!
            }
        }

        let mut disk_opt = self.disk_image.lock().await;
        if let Some(disk) = disk_opt.as_mut() {
            let data = self.track_handle.start_track_load();
            match embassy_futures::select::select(disk.read_track(desired_track, data), TRACK_CHANNEL.receive()).await {
                embassy_futures::select::Either::First(Ok(layout)) => {
                    self.pre_cached_ht = Some((desired_track, layout.0, layout.1, layout.2));
                    debug!("Pre-cached half-track {} into back buffer", desired_track);
                    return None;
                }
                embassy_futures::select::Either::First(Err(_)) => {
                    return None;
                }
                embassy_futures::select::Either::Second(new_ht) => {
                    return Some(new_ht);
                }
            }
        }
        None
    }

    async fn handle_track_event(&mut self, half_track: u8) {"""

content = content.replace("    async fn handle_track_event(&mut self, half_track: u8) {", new_method)

# Update handle_track_event logic
old_handle = """    async fn handle_track_event(&mut self, half_track: u8) {
        let mut target_ht = half_track;

        loop {
            // Drain queue
            while let Ok(ht) = TRACK_CHANNEL.try_receive() {
                target_ht = ht;
            }
            
            debug!("Manager loading half-track: {}", target_ht);

            // Prevent DOS from reading the old track data by stalling it safely.
            let old_layout = self.track_handle.get_active_layout();
            self.track_handle.set_active_layout(TrackLayout::Void);

            let mut disk_opt = self.disk_image.lock().await;
            if let Some(disk) = disk_opt.as_mut() {
                let data = self.track_handle.start_track_load();
                
                let start_time = embassy_time::Instant::now();
                // Read track but CANCEL if the head steps again!
                match select(disk.read_track(target_ht, data), TRACK_CHANNEL.receive()).await {
                    Either::First(Ok(layout)) => {
                        self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(target_ht, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(target_ht))), layout.0, layout.1);
                        let elapsed = start_time.elapsed().as_millis();
                        info!("Half-Track {} load took {} ms", target_ht, elapsed);
                        break; // Success, done loading the track!
                    }"""

new_handle = """    async fn handle_track_event(&mut self, half_track: u8) {
        let mut target_ht = half_track;

        loop {
            let start_ht = self.track_handle.get_head_half_track();
            
            // Drain queue
            while let Ok(ht) = TRACK_CHANNEL.try_receive() {
                target_ht = ht;
            }
            
            // Update direction
            let diff = target_ht as i16 - start_ht as i16;
            if diff != 0 {
                self.last_step_direction = if diff > 0 { 2 } else { -2 };
            }
            
            debug!("Manager loading half-track: {}", target_ht);

            // Check if it's already pre-cached in the back buffer!
            if let Some((ht, layout, length, speed_zone)) = self.pre_cached_ht {
                if ht == target_ht {
                    // It's perfectly ready in the back buffer. Just commit it!
                    self.track_handle.commit_loaded_track(
                        pico1541_common::track_buffer::TrackIdentifier::new(target_ht, speed_zone.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(target_ht))),
                        layout,
                        length
                    );
                    self.pre_cached_ht = None; // Used it!
                    info!("Half-Track {} load took 0 ms (PRE-CACHED)", target_ht);
                    break;
                }
            }
            
            // Not cached, we will use the back buffer for the new read.
            self.pre_cached_ht = None;

            // Prevent DOS from reading the old track data by stalling it safely.
            let old_layout = self.track_handle.get_active_layout();
            self.track_handle.set_active_layout(TrackLayout::Void);

            let mut disk_opt = self.disk_image.lock().await;
            if let Some(disk) = disk_opt.as_mut() {
                let data = self.track_handle.start_track_load();
                
                let start_time = embassy_time::Instant::now();
                // Read track but CANCEL if the head steps again!
                match select(disk.read_track(target_ht, data), TRACK_CHANNEL.receive()).await {
                    Either::First(Ok(layout)) => {
                        self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(target_ht, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(target_ht))), layout.0, layout.1);
                        let elapsed = start_time.elapsed().as_millis();
                        info!("Half-Track {} load took {} ms", target_ht, elapsed);
                        break; // Success, done loading the track!
                    }"""
content = content.replace(old_handle, new_handle)

# Add calling in run()
new_run_block = """                        self.track_handle.set_active_layout(old_layout);
                    }

                    if let Some(new_ht) = self.cache_adjacent_tracks().await {
                        self.handle_track_event(new_ht).await;
                    }
                }
                Either3::Third(cmd) => {"""
content = content.replace("                        self.track_handle.set_active_layout(old_layout);\n                    }\n                }\n                Either3::Third(cmd) => {", new_run_block)

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)

