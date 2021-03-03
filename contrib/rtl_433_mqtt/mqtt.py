import re
import sys
import json

import click
from paho.mqtt import client as mqtt


KLIMALOGG_MAP = {
    30180: 'office',
    31223: 'bedroom',
    31850: 'andres-room'
}


RADIOHEAD_MAP = {
    237: ('living-room', 'temperature'),
    238: ('living-room', 'humidity'),
    239: ('living-room', 'co2')
}


def handle_klimalogg(data):
    m = re.match(r'^([\d\.]+) C$', data['temperature_C'])
    sensor_id = data['id']
    room = KLIMALOGG_MAP[sensor_id]
    yield (room, 'temperature', m.group(1))
    yield (room, 'humidity', str(data['humidity']))


def handle_radiohead(data):
    pl = data['payload']
    (room, measure) = RADIOHEAD_MAP[data['id']]
    val = pl[1] * 256 + pl[0]

    if measure == 'temperature':
        val /= 100

    yield (room, measure, str(val))


def iter_stdin():
    for line in sys.stdin:
        data = json.loads(line)
        model = data.get('model')
        if model is None:
            continue
        
        if model.startswith('Klima'):
            yield from handle_klimalogg(data)
        else:
            yield from handle_radiohead(data)

@click.command()
@click.argument("host")
@click.option("--port", default=1883)
@click.option("--username")
@click.option("--password")
def main(host, port, username, password):
    client = mqtt.Client()

    if username and password:
        client.username_pw_set(username, password)
    client.connect(host, port=port)

    client.loop_start()

    for (room, sensor, value) in iter_stdin():
        (err, mid) = client.publish(f"home/sensors/{room}/{sensor}", value, qos=1)

        if err != mqtt.MQTT_ERR_SUCCESS:
            print(mqtt.error_string(err))
        # send msg


if __name__ == '__main__':
    main()
