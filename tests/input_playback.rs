// BLOCKED: add time strategy tests: https://github.com/bevyengine/bevy/issues/6146

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::utils::Duration;

use bevy::window::WindowPlugin;
use leafwing_input_playback::frame_counting::FrameCount;

use leafwing_input_playback::input_capture::InputCapturePlugin;
use leafwing_input_playback::input_capture::InputModesCaptured;
use leafwing_input_playback::input_playback::InputPlaybackPlugin;
use leafwing_input_playback::input_playback::PlaybackStrategy;
use leafwing_input_playback::timestamped_input::TimestampedInputs;

fn test_press(window: Entity) -> KeyboardInput {
    KeyboardInput {
        scan_code: 1,
        key_code: Some(KeyCode::F),
        state: ButtonState::Pressed,
        window,
    }
}

fn test_release(window: Entity) -> KeyboardInput {
    KeyboardInput {
        scan_code: 1,
        key_code: Some(KeyCode::F),
        state: ButtonState::Released,
        window,
    }
}

const TEST_KEY: KeyCode = KeyCode::F;

fn playback_app(strategy: PlaybackStrategy) -> App {
    let mut app = App::new();

    app
        .add_plugins((
            MinimalPlugins,
            WindowPlugin::default(),
            InputPlugin,
            InputPlaybackPlugin,
        ))
        // Bevy events are updated based on the FixedUpdate schedule. This migrates the
        // event buffers, changing the total `.len()` count. Since we are measuring the
        // number of events in some of these tests, we need to make FixedUpdate run
        // once every update.
        //
        // Alternatively, we could set ResMut<EventUpdateSignal> to true every Update,
        // but EventUpdateSignal is not available in Bevy's public API.
        .add_systems(Startup, (
            |mut time: ResMut<Time<Fixed>>| time.set_timestep(Duration::MAX),
        ))
        .add_systems(Last, |world: &mut World| {
            let _ = world.try_schedule_scope(FixedUpdate, |world, schedule| {
                schedule.run(world);
            });
        });

    *app.world.resource_mut::<PlaybackStrategy>() = strategy;

    app
}

fn simple_timestamped_input() -> TimestampedInputs {
    let mut inputs = TimestampedInputs::default();
    let window = Entity::from_raw(0);
    inputs.send(FrameCount(1), Duration::from_secs(0), test_press(window).into());
    inputs.send(FrameCount(2), Duration::from_secs(0), test_release(window).into());

    inputs
}

fn complex_timestamped_input() -> TimestampedInputs {
    let mut inputs = TimestampedInputs::default();
    let window = Entity::from_raw(0);
    inputs.send(FrameCount(0), Duration::from_secs(0), test_press(window).into());
    inputs.send(FrameCount(1), Duration::from_secs(1), test_release(window).into());
    inputs.send(FrameCount(2), Duration::from_secs(2), test_press(window).into());
    inputs.send(FrameCount(2), Duration::from_secs(3), test_press(window).into());
    inputs.send(FrameCount(3), Duration::from_secs(3), test_press(window).into());

    inputs
}

#[test]
fn minimal_playback() {
    let mut app = playback_app(PlaybackStrategy::FrameCount);
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 0);

    *app.world.resource_mut::<TimestampedInputs>() = simple_timestamped_input();
    app.update();

    // By default, only events up to the current frame are played back
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 1);
    let input = app.world.resource::<Input<KeyCode>>();
    assert!(input.pressed(KeyCode::F));

    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    // Events are double-buffered
    assert_eq!(input_events.len(), 2);
    let input = app.world.resource::<Input<KeyCode>>();
    assert!(!input.pressed(KeyCode::F));
}

#[test]
fn capture_and_playback() {
    let mut app = playback_app(PlaybackStrategy::default());
    app.add_plugins(InputCapturePlugin);
    app.insert_resource(PlaybackStrategy::Paused);

    let window = app.world.query::<(Entity, &Window)>().iter(&app.world).next().unwrap().0;

    let mut input_events = app.world.resource_mut::<Events<KeyboardInput>>();
    input_events.send(test_press(window));

    app.update();

    let input = app.world.resource::<Input<KeyCode>>();
    // Input is pressed because we just sent a real event
    assert!(input.pressed(TEST_KEY));

    app.update();
    let input = app.world.resource::<Input<KeyCode>>();
    // Input is not pressed, as playback is not enabled and the previous event expired
    assert!(input.pressed(TEST_KEY));

    app.insert_resource(InputModesCaptured::DISABLE_ALL);
    // This should trigger playback of input captured so far.
    app.insert_resource(PlaybackStrategy::FrameCount);

    app.update();

    let input = app.world.resource::<Input<KeyCode>>();
    // Input is now pressed, as the pressed key has been played back.
    assert!(input.pressed(TEST_KEY));
}

#[test]
fn repeated_playback() {
    // Play all of the events each pass
    let mut app = playback_app(PlaybackStrategy::default());
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 0);

    *app.world.resource_mut::<TimestampedInputs>() = simple_timestamped_input();
    for _ in 1..10 {
        app.update();
    }

    // Verify that we're out of events
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 0);

    // Reset our tracking
    let mut timestamped_input: Mut<TimestampedInputs> = app.world.resource_mut();
    timestamped_input.reset_cursor();

    // Play the events again
    app.update();

    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 2);
}

#[test]
fn playback_strategy_paused() {
    let mut app = playback_app(PlaybackStrategy::Paused);
    *app.world.resource_mut::<TimestampedInputs>() = complex_timestamped_input();

    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 0);

    for _ in 0..10 {
        app.update();
    }

    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 0);
}

#[test]
fn playback_strategy_frame() {
    let mut app = playback_app(PlaybackStrategy::FrameCount);
    *app.world.resource_mut::<TimestampedInputs>() = complex_timestamped_input();

    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 0);

    // Check complex_timestamped_input to verify the pattern
    app.update();
    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 2);

    app.update();
    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 4);

    app.update();
    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 5);
}

#[test]
fn playback_strategy_frame_range_once() {
    let strategy = PlaybackStrategy::FrameRangeOnce(FrameCount(2), FrameCount(5));
    let mut app = playback_app(strategy);
    *app.world.resource_mut::<TimestampedInputs>() = complex_timestamped_input();

    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 0);

    // Replays the events in the frame range [2, 5)
    // This playback strategy plays back the inputs one frame at a time until the entire range is captured
    // Then swaps to PlaybackStrategy::Paused
    // Frame 2
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 2);
    //input_events.read();

    // Frame 3 (events are double buffered)
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 3);

    // Frame 4 (events are double buffered)
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(*app.world.resource::<PlaybackStrategy>(), strategy);
    assert_eq!(input_events.len(), 1);

    // Paused
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 0);
    assert_eq!(
        *app.world.resource::<PlaybackStrategy>(),
        PlaybackStrategy::Paused
    );
}

#[test]
fn playback_strategy_frame_range_loop() {
    let strategy = PlaybackStrategy::FrameRangeLoop(FrameCount(2), FrameCount(5));
    let mut app = playback_app(strategy);
    *app.world.resource_mut::<TimestampedInputs>() = complex_timestamped_input();

    let timestamped_input = app.world.resource::<TimestampedInputs>();
    assert_eq!(timestamped_input.cursor, 0);

    // Replays the events in the frame range [2, 5)
    // This playback strategy plays back the inputs one frame at a time until the entire range is captured
    // Then swaps to PlaybackStrategy::Paused
    // Frame 2
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 2);

    // Frame 3 (events are double buffered)
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 3);

    // Frame 4 (events are double buffered)
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(*app.world.resource::<PlaybackStrategy>(), strategy);
    assert_eq!(input_events.len(), 1);

    // Spacing frame
    app.update();

    // Looping back to frame 2
    app.update();
    let input_events = app.world.resource::<Events<KeyboardInput>>();
    assert_eq!(input_events.len(), 2);
    assert_eq!(
        *app.world.resource::<PlaybackStrategy>(),
        PlaybackStrategy::FrameRangeLoop(FrameCount(2), FrameCount(5))
    );
}
