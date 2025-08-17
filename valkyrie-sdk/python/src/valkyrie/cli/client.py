#!/usr/bin/env python3
"""
Valkyrie Protocol Python Client CLI
"""

import asyncio
import json
import time
import click
import structlog
from typing import Dict, Any

from ..client import ValkyrieClient, ClientConfig
from ..errors import ValkyrieError

# Configure logging
structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.processors.UnicodeDecoder(),
        structlog.processors.JSONRenderer()
    ],
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    wrapper_class=structlog.stdlib.BoundLogger,
    cache_logger_on_first_use=True,
)

logger = structlog.get_logger(__name__)


@click.group()
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose logging')
@click.pass_context
def cli(ctx, verbose):
    """Valkyrie Protocol Python Client CLI"""
    ctx.ensure_object(dict)
    ctx.obj['verbose'] = verbose
    
    if verbose:
        import logging
        logging.basicConfig(level=logging.DEBUG)


@cli.command()
@click.option('--endpoint', '-e', default='tcp://localhost:8080', help='Server endpoint')
@click.option('--data', '-d', required=True, help='JSON data to send')
@click.option('--endpoint-path', '-p', default='/', help='Endpoint path')
@click.option('--timeout', '-t', default=30000, help='Request timeout in milliseconds')
@click.pass_context
async def request(ctx, endpoint, data, endpoint_path, timeout):
    """Send a request to the server"""
    try:
        # Parse JSON data
        try:
            request_data = json.loads(data)
        except json.JSONDecodeError as e:
            click.echo(f"Invalid JSON data: {e}", err=True)
            return
        
        # Create client
        config = ClientConfig(
            endpoint=endpoint,
            request_timeout_ms=timeout,
            enable_pooling=False
        )
        
        async with ValkyrieClient(config) as client:
            start_time = time.time()
            
            # Send request
            response = await client.request(request_data)
            
            end_time = time.time()
            duration_ms = (end_time - start_time) * 1000
            
            # Output response
            click.echo(json.dumps(response, indent=2))
            
            if ctx.obj.get('verbose'):
                click.echo(f"\nRequest completed in {duration_ms:.2f}ms", err=True)
    
    except ValkyrieError as e:
        click.echo(f"Valkyrie error: {e}", err=True)
        exit(1)
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        exit(1)


@cli.command()
@click.option('--endpoint', '-e', default='tcp://localhost:8080', help='Server endpoint')
@click.option('--data', '-d', required=True, help='JSON data to send')
@click.option('--endpoint-path', '-p', default='/', help='Endpoint path')
@click.pass_context
async def notify(ctx, endpoint, data, endpoint_path):
    """Send a notification to the server"""
    try:
        # Parse JSON data
        try:
            notification_data = json.loads(data)
        except json.JSONDecodeError as e:
            click.echo(f"Invalid JSON data: {e}", err=True)
            return
        
        # Create client
        config = ClientConfig(
            endpoint=endpoint,
            enable_pooling=False
        )
        
        async with ValkyrieClient(config) as client:
            start_time = time.time()
            
            # Send notification
            await client.notify(notification_data)
            
            end_time = time.time()
            duration_ms = (end_time - start_time) * 1000
            
            click.echo("Notification sent successfully")
            
            if ctx.obj.get('verbose'):
                click.echo(f"Notification sent in {duration_ms:.2f}ms", err=True)
    
    except ValkyrieError as e:
        click.echo(f"Valkyrie error: {e}", err=True)
        exit(1)
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        exit(1)


@cli.command()
@click.option('--endpoint', '-e', default='tcp://localhost:8080', help='Server endpoint')
@click.option('--count', '-c', default=100, help='Number of requests to send')
@click.option('--concurrency', '-n', default=10, help='Number of concurrent requests')
@click.option('--data', '-d', default='{"action": "ping"}', help='JSON data to send')
@click.pass_context
async def benchmark(ctx, endpoint, count, concurrency, data):
    """Benchmark server performance"""
    try:
        # Parse JSON data
        try:
            request_data = json.loads(data)
        except json.JSONDecodeError as e:
            click.echo(f"Invalid JSON data: {e}", err=True)
            return
        
        # Create client
        config = ClientConfig(
            endpoint=endpoint,
            max_connections=concurrency,
            enable_pooling=True
        )
        
        async with ValkyrieClient(config) as client:
            click.echo(f"Benchmarking {endpoint}")
            click.echo(f"Requests: {count}, Concurrency: {concurrency}")
            click.echo("Starting benchmark...")
            
            # Benchmark function
            async def send_request():
                try:
                    start = time.time()
                    await client.request(request_data)
                    end = time.time()
                    return (end - start) * 1000, None
                except Exception as e:
                    return None, str(e)
            
            # Run benchmark
            start_time = time.time()
            
            # Create semaphore for concurrency control
            semaphore = asyncio.Semaphore(concurrency)
            
            async def bounded_request():
                async with semaphore:
                    return await send_request()
            
            # Execute all requests
            tasks = [bounded_request() for _ in range(count)]
            results = await asyncio.gather(*tasks)
            
            end_time = time.time()
            total_duration = end_time - start_time
            
            # Analyze results
            successful_requests = [r for r in results if r[0] is not None]
            failed_requests = [r for r in results if r[0] is None]
            
            if successful_requests:
                latencies = [r[0] for r in successful_requests]
                avg_latency = sum(latencies) / len(latencies)
                min_latency = min(latencies)
                max_latency = max(latencies)
                
                # Calculate percentiles
                sorted_latencies = sorted(latencies)
                p50 = sorted_latencies[int(len(sorted_latencies) * 0.5)]
                p95 = sorted_latencies[int(len(sorted_latencies) * 0.95)]
                p99 = sorted_latencies[int(len(sorted_latencies) * 0.99)]
                
                requests_per_second = len(successful_requests) / total_duration
                
                # Output results
                click.echo("\n" + "="*50)
                click.echo("BENCHMARK RESULTS")
                click.echo("="*50)
                click.echo(f"Total requests: {count}")
                click.echo(f"Successful requests: {len(successful_requests)}")
                click.echo(f"Failed requests: {len(failed_requests)}")
                click.echo(f"Total time: {total_duration:.2f}s")
                click.echo(f"Requests per second: {requests_per_second:.2f}")
                click.echo()
                click.echo("LATENCY STATISTICS (ms)")
                click.echo("-" * 30)
                click.echo(f"Average: {avg_latency:.2f}")
                click.echo(f"Minimum: {min_latency:.2f}")
                click.echo(f"Maximum: {max_latency:.2f}")
                click.echo(f"50th percentile: {p50:.2f}")
                click.echo(f"95th percentile: {p95:.2f}")
                click.echo(f"99th percentile: {p99:.2f}")
                
                if failed_requests and ctx.obj.get('verbose'):
                    click.echo("\nERRORS:")
                    error_counts = {}
                    for _, error in failed_requests:
                        error_counts[error] = error_counts.get(error, 0) + 1
                    
                    for error, count in error_counts.items():
                        click.echo(f"  {error}: {count}")
            else:
                click.echo("All requests failed!")
                if ctx.obj.get('verbose'):
                    for _, error in failed_requests[:10]:  # Show first 10 errors
                        click.echo(f"  Error: {error}")
    
    except ValkyrieError as e:
        click.echo(f"Valkyrie error: {e}", err=True)
        exit(1)
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        exit(1)


@cli.command()
@click.option('--endpoint', '-e', default='tcp://localhost:8080', help='Server endpoint')
@click.pass_context
async def ping(ctx, endpoint):
    """Ping the server"""
    try:
        config = ClientConfig(
            endpoint=endpoint,
            enable_pooling=False
        )
        
        async with ValkyrieClient(config) as client:
            rtt_ms = await client.ping()
            click.echo(f"Ping successful: {rtt_ms:.2f}ms")
    
    except ValkyrieError as e:
        click.echo(f"Ping failed: {e}", err=True)
        exit(1)
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        exit(1)


def main():
    """Main entry point"""
    # Convert sync click commands to async
    import sys
    
    def run_async(coro):
        return asyncio.run(coro)
    
    # Patch click commands to be async
    for command in cli.commands.values():
        if asyncio.iscoroutinefunction(command.callback):
            original_callback = command.callback
            command.callback = lambda *args, **kwargs: run_async(original_callback(*args, **kwargs))
    
    cli()


if __name__ == '__main__':
    main()