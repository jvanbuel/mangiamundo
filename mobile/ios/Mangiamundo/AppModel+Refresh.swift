import Foundation

/// Loading the area's progress, and the reminders that follow from it.
extension AppModel {
    /// Asks for the server's version until it answers once.
    func loadServer() {
        guard server == nil || server == "not reached" else { return }
        Task {
            if let health = try? await api.health() {
                server = [health.version ?? "unknown", health.commit].compactMap { $0 }.joined(separator: " ")
            } else {
                server = "not reached"
            }
        }
    }

    func refresh() {
        loadServer()
        guard let here = location else { return }
        refreshTask?.cancel()
        refreshes += 1
        let refresh = refreshes
        refreshTask = Task {
            loading = true
            error = nil
            // A wheel from the previous area must not linger while this one starts.
            areaProgress = nil
            // A new area can take a while to load: show the server's progress after 0.3 s.
            let poll = Task {
                try? await Task.sleep(for: .milliseconds(300))
                while !Task.isCancelled {
                    // nil (not started or already done) and errors keep the last value. Only the
                    // latest refresh shows its wheel.
                    if let progress = try? await api.areaProgress(areaID: here.area.id), !Task.isCancelled, refresh == refreshes {
                        areaProgress = progress
                        // At startup the loader waits for the data: let the wheel show instead.
                        ready = true
                    }
                    try? await Task.sleep(for: .milliseconds(400))
                }
            }
            let fetched: Result<Discover, Error>
            do {
                fetched = .success(try await api.discover(areaID: here.area.id))
            } catch {
                fetched = .failure(error)
            }
            // The wheel is about the restaurants: it goes before celebrations and reminders load.
            poll.cancel()
            if refresh == refreshes { areaProgress = nil }
            do {
                let result = try fetched.get()
                guard !Task.isCancelled else { return }
                discover = result
                ready = true
                if let code = pendingCountry {
                    pendingCountry = nil
                    if result.countries.contains(where: { $0.code == code }) { select(code) }
                }
                celebrations = (try? await api.celebrations(areaID: here.area.id)) ?? []
                await announceConfirmedAdditions()
                // On its own, so loading doesn't wait for it.
                Task { await rescheduleReminders() }
            } catch {
                guard !Task.isCancelled else { return }
                self.error = error.localizedDescription
                discover = nil
                ready = true
            }
            loading = false
        }
    }

    /// Opens what a notification points at.
    func openFromNotification(country: String?, badges: Bool) {
        if badges {
            select(nil)
            tab = 1
        }
        guard let country else { return }
        if discover?.countries.contains(where: { $0.code == country }) == true {
            select(country)
        } else {
            pendingCountry = country
        }
    }

    func toggleReminders() async {
        if remindersOn {
            remindersOn = false
            await reminders.cancelAll()
        } else if await reminders.requestPermission() {
            remindersOn = true
            await rescheduleReminders()
            toast("Reminders on. We will let you know when a country near you has something to celebrate.")
        } else {
            toast("Notifications are turned off for Mangiamundo in Settings.")
        }
        defaults.set(remindersOn, forKey: "mangiamundo.reminders")
    }

    /// Schedules what the server plans for the area. Without an answer, what was scheduled stays.
    private func rescheduleReminders() async {
        guard remindersOn, let here = location else { return }
        guard let plan = try? await api.reminders(areaID: here.area.id) else { return }
        // Turned off or moved meanwhile: this plan is no longer wanted.
        guard remindersOn, location?.area.id == here.area.id else { return }
        await reminders.schedule(ReminderSchedule.requests(for: plan, now: Date()))
    }
}
