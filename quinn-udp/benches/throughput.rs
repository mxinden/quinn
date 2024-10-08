use criterion::{criterion_group, criterion_main, Criterion};
use quinn_udp::{RecvMeta, Transmit, UdpSocketState, BATCH_SIZE};
use tokio::io::Interest;
use std::{cmp::min, io::{ErrorKind, IoSliceMut}, net::UdpSocket};
use tokio::runtime::Runtime;

pub fn criterion_benchmark(c: &mut Criterion) {
    const TOTAL_BYTES: usize = 10 * 1024 * 1024;
    const SEGMENT_SIZE: usize = 1280;

    let mut permutations = vec![];
    for gso_enabled in [false, true] {
        for gro_enabled in [false, true] {
            for recvmmsg_enabled in [false, true] {
                permutations.push((gso_enabled, gro_enabled, recvmmsg_enabled))
            }
        }
    }

    for (gso_enabled, gro_enabled, recvmmsg_enabled) in permutations {
        let mut group = c.benchmark_group(format!("gso_{}_gro_{}_recvmmsg_{}", gso_enabled, gro_enabled, recvmmsg_enabled));
        group.throughput(criterion::Throughput::Bytes(TOTAL_BYTES as u64));


        let send = UdpSocket::bind("[::1]:0")
            .or_else(|_| UdpSocket::bind("127.0.0.1:0"))
            .unwrap();
        let recv = UdpSocket::bind("[::1]:0")
            .or_else(|_| UdpSocket::bind("127.0.0.1:0"))
            .unwrap();
        let dst_addr = recv.local_addr().unwrap();
        let send_state = UdpSocketState::new((&send).into()).unwrap();
        let recv_state = UdpSocketState::new((&recv).into()).unwrap();

        // TODO: Min needed?
        let gso_segments = min(32, if gso_enabled { send_state.max_gso_segments() } else { 1 });
        // TODO: Min needed?
        let msg = vec![0xAB; min(u16::MAX as usize, SEGMENT_SIZE * dbg!(gso_segments))];

        let transmit = Transmit {
            destination: dst_addr,
            ecn: None,
            contents: &msg,
            segment_size: gso_enabled.then_some(SEGMENT_SIZE),
            src_ip: None,
        };

        group.bench_function("throughput", |b| {
            b.to_async(Runtime::new().unwrap()).iter(
                || async {
                    let send = tokio::net::UdpSocket::from_std(send.try_clone().unwrap()).unwrap();
                    let recv = tokio::net::UdpSocket::from_std(recv.try_clone().unwrap()).unwrap();

                    let gro_segments = min(32, if gro_enabled { recv_state.gro_segments() } else { 1 });
                    let batch_size = if recvmmsg_enabled { BATCH_SIZE } else { 1 };
                    let mut receive_buffers = vec![vec![0; SEGMENT_SIZE * dbg!(gro_segments) ]; dbg!(batch_size)];
                    println!("{}", receive_buffers[0].len());
                    let mut receive_slices = receive_buffers
                        .iter_mut()
                        .map(|buf| IoSliceMut::new(buf))
                        .collect::<Vec<_>>();
                    let mut meta = vec![RecvMeta::default(); batch_size];

                    let mut sent: usize = 0;
                    let mut received: usize = 0;
                    while dbg!(sent) < TOTAL_BYTES {
                        send.writable().await.unwrap();
                        send.try_io(Interest::WRITABLE, || {
                            send_state.send((&send).into(), &transmit)
                        }).unwrap();
                        sent += transmit.contents.len();

                        while dbg!(received) < dbg!(sent){
                            recv.readable().await.unwrap();
                            let n = match recv.try_io(Interest::READABLE, || {
                                recv_state
                                    .recv((&recv).into(), &mut receive_slices, &mut meta)
                            }) {
                                Ok(n) => n,
                                // false positive.
                                Err(e) if e.kind() == ErrorKind::WouldBlock => {println!("continue"); continue},
                                e => e.unwrap(),
                            };
                            for i in meta.iter().take(dbg!(n)) {
                                received += i.len;
                            }
                        }
                    }
                },
            )
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
