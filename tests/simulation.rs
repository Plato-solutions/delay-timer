use delay_timer::prelude::*;

use std::str::FromStr;
use std::sync::atomic::{
    AtomicUsize,
    Ordering::{Acquire, Release},
};
use std::sync::{
    atomic::{AtomicI32, AtomicU64},
    Arc,
};
use std::thread::{self, park_timeout};
use std::time::Duration;

use smol::Timer;

#[test]
fn test_instance_state() -> anyhow::Result<()> {
    let delay_timer = DelayTimer::new();

    let body = create_async_fn_body!({
        Timer::after(Duration::from_millis(100)).await;
    });

    let task = TaskBuilder::default()
        .set_frequency_by_candy(CandyFrequency::CountDown(4, CandyCron::Secondly))
        .set_task_id(1)
        .set_maximun_parallel_runable_num(3)
        .spawn(body)?;
    let task_instance_chain = delay_timer.insert_task(task)?;

    // Get the first task instance.
    let instance = task_instance_chain.next_with_wait()?;

    // The task was still running when the instance was first obtained.
    assert_eq!(instance.get_state(), instance::RUNNING);

    // Unsolicited mission cancellation.
    instance.cancel_with_wait()?;
    assert_eq!(instance.get_state(), instance::CANCELLED);

    // Get the second task instance.
    let instance = task_instance_chain.next_with_wait()?;

    // Just got the instance when it was still running.
    assert_eq!(instance.get_state(), instance::RUNNING);

    // The task execution is completed after about 110 millisecond.
    park_timeout(Duration::from_millis(110));

    // This should be the completed state.
    assert_eq!(instance.get_state(), instance::COMPLETED);

    Ok(())
}

#[test]
fn test_instance_timeout_state() -> anyhow::Result<()> {
    let delay_timer = DelayTimer::new();

    let body = create_async_fn_body!({
        Timer::after(Duration::from_secs(3)).await;
    });

    let task = TaskBuilder::default()
        .set_frequency_by_candy(CandyFrequency::CountDown(4, CandyCron::Secondly))
        .set_task_id(1)
        .set_maximum_running_time(2)
        .set_maximun_parallel_runable_num(3)
        .spawn(body)?;
    let task_instance_chain = delay_timer.insert_task(task)?;

    // Get the first task instance.
    let instance = task_instance_chain.next_with_wait()?;

    // The task was still running when the instance was first obtained.
    assert_eq!(instance.get_state(), instance::RUNNING);

    // The task execution is timeout after about 2.001 second.
    park_timeout(Duration::from_millis(2001));

    // This should be the completed state.
    assert_eq!(instance.get_state(), instance::TIMEOUT);

    Ok(())
}

#[cfg(replace_shell_command)]
#[test]
fn test_shell_task_instance_timeout_state() -> anyhow::Result<()> {
    let delay_timer = DelayTimer::new();

    // Before doing this test, please make sure that your local machine can execute this command.
    let shell_command = "php /home/open/project/rust/repo/myself/delay_timer/examples/try_spawn.php >> ./try_spawn.txt";
    let body = unblock_process_task_fn(shell_command.into());

    let task = TaskBuilder::default()
        .set_frequency_by_candy(CandyFrequency::Repeated(CandyCron::Secondly))
        .set_task_id(3)
        .set_maximum_running_time(3)
        .set_maximun_parallel_runable_num(1)
        .spawn(body)?;

    let task_instance_chain = delay_timer.insert_task(task)?;

    // Get the first task instance.
    let instance = task_instance_chain.next_with_wait()?;

    // The task was still running when the instance was first obtained.
    assert_eq!(instance.get_state(), instance::RUNNING);

    // The task still running after about 2.001 second.
    park_timeout(Duration::from_millis(2001));
    assert_eq!(instance.get_state(), instance::RUNNING);

    // This should be the timeout state after about 4.001 second.
    park_timeout(Duration::from_millis(2000));

    assert_eq!(instance.get_state(), instance::TIMEOUT);

    Ok(())
}

#[cfg(replace_shell_command)]
#[test]
fn test_shell_task_instance_complete_state() -> anyhow::Result<()> {
    let mut delay_timer = DelayTimerBuilder::default().enable_status_report().build();
    let status_reporter = delay_timer
        .take_status_reporter()
        .ok_or(anyhow!("Without `status_reporter`."))?;

    // Before doing this test, please make sure that your local machine can execute this command.
    let shell_command = "php -v";
    let body = unblock_process_task_fn(shell_command.into());

    let task = TaskBuilder::default()
        .set_frequency_by_candy(CandyFrequency::Repeated(CandyCron::Secondly))
        .set_task_id(3)
        .set_maximum_running_time(3)
        .set_maximun_parallel_runable_num(1)
        .spawn(body)?;

    let task_instance_chain = delay_timer.insert_task(task)?;

    // Get the first task instance.
    let instance = task_instance_chain.next_with_wait()?;

    // The task was still running when the instance was first obtained.
    assert_eq!(instance.get_state(), instance::RUNNING);

    let _running_event = status_reporter.next_public_event_with_wait()?;
    let complete_event = status_reporter.next_public_event_with_wait()?;

    dbg!(complete_event);

    // This should be the completed state.
    assert!(instance.get_state() >= instance::COMPLETED);

    Ok(())
}

#[test]
fn go_works() -> AnyResult<()> {
    // Coordinates the inner-Runtime with the external(test-thread) clock.
    let expression = "0/2 * * * * * *";
    let park_time = 2_100_000u64;

    let delay_timer = DelayTimer::new();
    let share_num = Arc::new(AtomicUsize::new(0));
    let share_num_bunshin = share_num.clone();

    let body = move |_| {
        share_num_bunshin.fetch_add(1, Release);
        create_default_delay_task_handler()
    };

    let task = TaskBuilder::default()
        .set_frequency(Frequency::CountDown(3, expression))
        .set_task_id(1)
        .spawn(body)?;
    delay_timer.add_task(task)?;

    for _ in 0..5 {
        //Testing, whether the mission is performing as expected.
        dbg!(share_num.load(Acquire));
        park_timeout(Duration::from_micros(park_time));
    }

    assert_eq!(share_num.load(Acquire), 3);
    Ok(())
}

#[test]
fn test_advance() -> AnyResult<()> {
    // The task is executed in the next hour.
    let expression = "@yearly";
    let task_id = 1;

    let delay_timer = DelayTimer::new();
    let share_num = Arc::new(AtomicUsize::new(0));
    let share_num_bunshin = share_num.clone();

    let body = move |_| {
        share_num_bunshin.fetch_add(1, Release);
        create_default_delay_task_handler()
    };

    let task = TaskBuilder::default()
        .set_frequency(Frequency::CountDown(3, expression))
        .set_task_id(task_id)
        .spawn(body)?;

    delay_timer.add_task(task)?;

    for i in 0..3 {
        assert_eq!(dbg!(share_num.load(Acquire)), i);
        // Go ahead.
        delay_timer.advance_task(task_id)?;

        // Waiting for scheduling and execution.
        park_timeout(Duration::from_secs(2));
    }
    Ok(())
}


#[test]
fn test_maximun_parallel_runable_num() -> AnyResult<()> {
    let delay_timer = DelayTimer::new();
    let share_num = Arc::new(AtomicU64::new(0));
    let share_num_bunshin = share_num.clone();

    let body = create_async_fn_body!((share_num_bunshin){
        dbg!();
        share_num_bunshin_ref.fetch_add(1, Release);
        Timer::after(Duration::from_secs(9)).await;
        share_num_bunshin_ref.fetch_sub(1, Release);
    });

    let task = TaskBuilder::default()
        .set_frequency_by_candy(CandyFrequency::CountDown(4, CandyCron::Secondly))
        .set_task_id(1)
        .set_maximun_parallel_runable_num(3)
        .spawn(body)?;
    delay_timer.add_task(task)?;

    for i in 1..=6 {
        park_timeout(Duration::from_micros(1_000_000 * i));

        //Testing, whether the mission is performing as expected.
        debug_assert!(dbg!(share_num.load(Acquire)) <= i);
    }

    Ok(())
}

#[test]
fn tests_countdown() -> AnyResult<()> {
    let delay_timer = DelayTimer::new();
    let share_num = Arc::new(AtomicI32::new(3));
    let share_num_bunshin = share_num.clone();
    let body = move |_| {
        share_num_bunshin.fetch_sub(1, Release);
        create_default_delay_task_handler()
    };

    let task = TaskBuilder::default()
        .set_frequency(Frequency::CountDown(3, "0/2 * * * * * *"))
        .set_task_id(1)
        .spawn(body)?;
    delay_timer.add_task(task)?;

    let mut i = 0;

    loop {
        i = i + 1;
        park_timeout(Duration::from_secs(3));

        if i == 6 {
            //The task runs 3 times, once per second, and after 6 seconds it goes down to 0 at most.
            assert_eq!(0, share_num.load(Acquire));
            break;
        }
    }
    Ok(())
}
#[test]
fn inspect_struct() -> AnyResult<()> {
    use tokio::runtime::Runtime;

    println!("Task size :{:?}", std::mem::size_of::<Task>());
    println!("Frequency size :{:?}", std::mem::size_of::<Frequency>());
    println!("TaskBuilder size :{:?}", std::mem::size_of::<TaskBuilder>());
    println!("DelayTimer size :{:?}", std::mem::size_of::<DelayTimer>());
    println!("Runtime size :{:?}", std::mem::size_of::<Runtime>());

    println!(
        "ScheduleIteratorOwned size :{:?}",
        std::mem::size_of::<cron_clock::ScheduleIteratorOwned<cron_clock::Utc>>()
    );

    let mut s = cron_clock::Schedule::from_str("* * * * * * *")?.upcoming_owned(cron_clock::Utc);

    let mut s1 = s.clone();

    println!("{:?}, {:?}", s.next(), s1.next());
    thread::sleep(Duration::from_secs(1));
    println!("{:?}, {:?}", s.next(), s1.next());
    let mut s2 = s1.clone();
    thread::sleep(Duration::from_secs(1));
    println!("{:?}, {:?}", s.next(), s2.next());
    Ok(())
}
