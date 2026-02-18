trigger SmokeTest on Account (before insert) {
    if (Trigger.new.size() > 0) {
        System.debug('Smoke trigger active');
    }
}
