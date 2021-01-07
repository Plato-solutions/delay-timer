#![feature(ptr_internals)]
use delay_timer::prelude::*;

use std::str::FromStr;
use std::sync::atomic::{
    AtomicUsize,
    Ordering::{Acquire, Release},
};
use std::sync::{atomic::AtomicI32, Arc};
use std::thread::{self, park_timeout};
use std::time::Duration;

use smol::Timer;
#[test]
fn go_works() {
    let delay_timer = DelayTimer::new();
    let share_num = Arc::new(AtomicUsize::new(0));
    let share_num_bunshin = share_num.clone();

    let body = move |_| {
        share_num_bunshin.fetch_add(1, Release);
        create_default_delay_task_handler()
    };

    let task = TaskBuilder::default()
        .set_frequency(Frequency::CountDown(3, "0/6 * * * * * *"))
        .set_task_id(1)
        .spawn(body)
        .unwrap();
    delay_timer.add_task(task).unwrap();

    let mut i = 0;

    loop {
        park_timeout(Duration::from_micros(6_100_000));

        //Testing, whether the mission is performing as expected.
        i = i + 1;
        assert_eq!(i, share_num.load(Acquire));

        if i == 3 {
            break;
        }
    }
}

#[test]
fn test_maximun_parallel_runable_num() {
    let delay_timer = DelayTimer::new();
    let share_num = Arc::new(AtomicUsize::new(0));
    let share_num_bunshin = share_num.clone();

    // FIXME:Write a new macro to support the generation of a `Fn` closure
    // that requires (multiple calls and consumes a copy of the capture variable ownership)
    // let body = create_async_fn_body!({
    //     share_num_bunshin.fetch_add(1, Release);
    //     Timer::after(Duration::from_secs(9))
    // });

    let body = move |context: TaskContext| {
        let share_num_bunshin_ref = share_num_bunshin.clone();
        let f = async move {
            let future_inner = async move {
                dbg!();
                share_num_bunshin_ref.fetch_add(1, Release);
                Timer::after(Duration::from_secs(9)).await;
                share_num_bunshin_ref.fetch_sub(1, Release);
            };
            future_inner.await;

            context.finishe_task().await;
        };
        let handle = async_spawn(f);
        create_delay_task_handler(handle)
    };
    let task = TaskBuilder::default()
        .set_frequency(Frequency::CountDown(9, "* * * * * * *"))
        .set_task_id(1)
        .set_maximun_parallel_runable_num(3)
        .spawn(body)
        .unwrap();
    delay_timer.add_task(task).unwrap();

    for _ in 0..3{
        park_timeout(Duration::from_micros(3_000_100));

        //Testing, whether the mission is performing as expected.
        debug_assert_eq!(3, share_num.load(Acquire));
    }
}

#[test]
fn tests_countdown() {
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
        .spawn(body)
        .unwrap();
    delay_timer.add_task(task).unwrap();

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
}
#[test]
fn inspect_struct() {
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

    let mut s = cron_clock::Schedule::from_str("* * * * * * *")
        .unwrap()
        .upcoming_owned(cron_clock::Utc);

    let mut s1 = s.clone();

    println!("{:?}, {:?}", s.next(), s1.next());
    thread::sleep(Duration::from_secs(1));
    println!("{:?}, {:?}", s.next(), s1.next());
    let mut s2 = s1.clone();
    thread::sleep(Duration::from_secs(1));
    println!("{:?}, {:?}", s.next(), s2.next());
}
